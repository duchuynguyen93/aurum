//! Watches the music folders and tells `AppState` when a rescan is worth running.

use std::path::PathBuf;
use std::time::Duration;

use aurum_core::library::has_audio_extension;
use crossbeam_channel::{Receiver, Sender};
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

/// Events are held back this long so copying an album triggers one scan, not hundreds.
const DEBOUNCE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchOutcome {
    Quiet,
    /// Something that can change the library happened.
    Changed,
    /// The OS dropped events or reported an error; the watcher should be rebuilt and a scan run.
    Broken,
}

pub struct Watch {
    debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    rx: Receiver<DebounceEventResult>,
    tx: Sender<DebounceEventResult>,
}

impl Watch {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            debouncer: None,
            rx,
            tx,
        }
    }

    /// Drops the old watcher and watches every root recursively. A root that cannot be watched
    /// is skipped (logged).
    pub fn rebuild(&mut self, roots: &[PathBuf]) {
        self.debouncer = None;
        if roots.is_empty() {
            return;
        }
        let mut debouncer = match new_debouncer(DEBOUNCE, None, self.tx.clone()) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("cannot start folder watcher: {e}");
                return;
            }
        };
        for root in roots {
            if let Err(e) = debouncer.watch(root, RecursiveMode::Recursive) {
                log::warn!("cannot watch {}: {e}", root.display());
            }
        }
        self.debouncer = Some(debouncer);
    }

    pub fn disable(&mut self) {
        self.debouncer = None;
    }

    /// Drains pending events. `Broken` wins over `Changed`.
    pub fn poll(&mut self) -> WatchOutcome {
        let mut outcome = WatchOutcome::Quiet;
        while let Ok(result) = self.rx.try_recv() {
            match result {
                Err(errors) => {
                    for e in errors {
                        log::warn!("folder watcher error: {e}");
                    }
                    outcome = WatchOutcome::Broken;
                }
                Ok(events) => {
                    for event in events {
                        if event.need_rescan() {
                            outcome = WatchOutcome::Broken;
                        } else if outcome == WatchOutcome::Quiet && is_relevant(&event) {
                            outcome = WatchOutcome::Changed;
                        }
                    }
                }
            }
        }
        outcome
    }
}

/// Audio files, or folder-level changes (a moved or deleted folder carries files with it).
fn is_relevant(event: &notify::Event) -> bool {
    if event.paths.iter().any(|p| has_audio_extension(p)) {
        return true;
    }
    match event.kind {
        EventKind::Create(CreateKind::Folder | CreateKind::Any)
        | EventKind::Remove(RemoveKind::Folder | RemoveKind::Any)
        | EventKind::Modify(ModifyKind::Name(_))
        | EventKind::Any => true,
        // FSEvents folds a deleted folder with its files into one metadata event on the
        // folder, so a vanished or directory path counts as a folder-level change.
        EventKind::Modify(ModifyKind::Metadata(_)) => {
            event.paths.iter().any(|p| !p.exists() || p.is_dir())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn wait_for(watch: &mut Watch, want: WatchOutcome, secs: u64) -> bool {
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            if watch.poll() == want {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    }

    #[test]
    fn audio_file_is_reported_and_text_file_is_not() {
        // Canonical: FSEvents reports resolved paths (/private/var on macOS).
        let dir = std::env::temp_dir().join(format!("aurum-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let mut watch = Watch::new();
        watch.rebuild(std::slice::from_ref(&dir));

        std::fs::write(dir.join("a.flac"), b"fLaC").unwrap();
        assert!(
            wait_for(&mut watch, WatchOutcome::Changed, 10),
            "no Changed after creating a.flac"
        );

        // Let the debounce window of the first batch drain before the negative check.
        std::thread::sleep(Duration::from_secs(3));
        watch.poll();
        std::fs::write(dir.join("b.txt"), b"hello").unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert_eq!(watch.poll(), WatchOutcome::Quiet);

        // A whole folder with audio inside disappears: FSEvents reports little more than that
        // the folder changed.
        let album = dir.join("album");
        std::fs::create_dir_all(&album).unwrap();
        std::fs::write(album.join("c.flac"), b"fLaC").unwrap();
        assert!(
            wait_for(&mut watch, WatchOutcome::Changed, 10),
            "no Changed after creating a folder"
        );
        std::thread::sleep(Duration::from_secs(3));
        watch.poll();
        std::fs::remove_dir_all(&album).unwrap();
        assert!(
            wait_for(&mut watch, WatchOutcome::Changed, 10),
            "no Changed after removing a folder"
        );

        watch.disable();
        std::fs::remove_dir_all(dir).ok();
    }
}
