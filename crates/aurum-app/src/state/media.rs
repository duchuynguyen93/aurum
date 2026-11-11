//! OS media controls (SMTC on Windows, Now Playing on macOS, MPRIS on Linux) through souvlaki.
//! Every failure here is logged and swallowed: media keys are a convenience, never a reason to
//! stop the player.

use std::path::Path;
use std::time::{Duration, Instant};

use aurum_core::Track;
use crossbeam_channel::Receiver;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

/// Timeline updates that are not caused by a user action are sent at most this often.
const TIMELINE_INTERVAL: Duration = Duration::from_secs(1);

pub struct Media {
    controls: Option<MediaControls>,
    rx: Receiver<MediaControlEvent>,
    last_timeline: Instant,
}

impl Media {
    pub fn new(window: &gpui_kit::Window) -> Media {
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut media = Media {
            controls: None,
            rx,
            last_timeline: Instant::now(),
        };

        let hwnd = match HasWindowHandle::window_handle(window) {
            Ok(h) => match h.as_raw() {
                RawWindowHandle::Win32(w) => Some(w.hwnd.get() as *mut std::ffi::c_void),
                _ => None,
            },
            Err(_) => None,
        };
        if cfg!(target_os = "windows") && hwnd.is_none() {
            // souvlaki panics on Windows without a window handle.
            log::warn!("media controls disabled: no Win32 window handle");
            return media;
        }

        let mut controls = match MediaControls::new(PlatformConfig {
            display_name: "Aurum",
            dbus_name: "aurum",
            hwnd,
        }) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("media controls unavailable: {e:?}");
                return media;
            }
        };
        if let Err(e) = controls.attach(move |event| {
            let _ = tx.send(event);
        }) {
            log::warn!("media controls cannot attach: {e:?}");
            return media;
        }
        log::info!("media controls ready");
        media.controls = Some(controls);
        media
    }

    /// Events the OS sent since the last call.
    pub fn poll(&mut self) -> Vec<MediaControlEvent> {
        self.rx.try_iter().collect()
    }

    /// Publishes the current track (or clears it) together with its cover thumbnail file.
    pub fn set_track(&mut self, track: Option<&Track>, cover: Option<&Path>) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        let result = match track {
            Some(track) => {
                let fallback;
                let title = if track.title.trim().is_empty() {
                    fallback = track
                        .path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    fallback.as_str()
                } else {
                    track.title.as_str()
                };
                let cover_url = cover.and_then(cover_url);
                controls.set_metadata(MediaMetadata {
                    title: Some(title),
                    artist: track.artist.as_deref(),
                    album: track.album.as_deref(),
                    duration: Some(Duration::from_millis(track.duration_ms)),
                    cover_url: cover_url.as_deref(),
                })
            }
            None => controls
                .set_metadata(MediaMetadata::default())
                .and_then(|()| controls.set_playback(MediaPlayback::Stopped)),
        };
        if let Err(e) = result {
            log::debug!("media metadata: {e:?}");
        }
    }

    /// Publishes play state and position. Unless `force`, at most once per second.
    pub fn set_playback(&mut self, playing: bool, position: Duration, force: bool) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        if !force && self.last_timeline.elapsed() < TIMELINE_INTERVAL {
            return;
        }
        self.last_timeline = Instant::now();
        let progress = Some(MediaPosition(position));
        let state = if playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        };
        if let Err(e) = controls.set_playback(state) {
            log::debug!("media playback: {e:?}");
        }
    }
}

/// URL of a local image file as each platform's media controls expect it.
fn cover_url(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        Some(format!("file://{}", path.display()))
    }
    #[cfg(not(windows))]
    {
        url::Url::from_file_path(path).ok().map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn cover_url_escapes_spaces() {
        let url = cover_url(Path::new("/tmp/my covers/a b.jpg")).unwrap();
        assert!(url.starts_with("file:///"), "{url}");
        assert!(url.contains("%20"), "{url}");
    }

    #[cfg(windows)]
    #[test]
    fn cover_url_keeps_raw_spaces() {
        let url = cover_url(Path::new(r"C:\my covers\a b.jpg")).unwrap();
        assert!(url.starts_with("file://"), "{url}");
        assert!(url.contains(' '), "{url}");
    }
}
