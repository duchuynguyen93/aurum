//! Album cover cache: `(artist, album)` to a decoded-on-demand GPUI image, bounded LRU.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use aurum_core::Track;
use aurum_core::library::cover;
use gpui_kit::{Image, ImageFormat};

pub const CAPACITY: usize = 200;

pub type CoverKey = (String, String);

/// Cache key of a track: album artist (or artist) plus album. Tracks without an album are keyed
/// by their own path so unrelated singles never share a cover.
pub fn key_of(track: &Track) -> CoverKey {
    match &track.album {
        Some(album) => (
            track
                .album_artist
                .clone()
                .or_else(|| track.artist.clone())
                .unwrap_or_default(),
            album.clone(),
        ),
        None => (String::new(), track.path.to_string_lossy().into_owned()),
    }
}

#[derive(Default)]
pub struct CoverCache {
    /// `None` records "this album has no usable cover" so it is not read again.
    entries: HashMap<CoverKey, Option<Arc<Image>>>,
    /// Least recently used first.
    order: VecDeque<CoverKey>,
    pending: HashSet<CoverKey>,
}

impl CoverCache {
    pub fn get(&mut self, key: &CoverKey) -> Option<Arc<Image>> {
        let image = self.entries.get(key)?.clone();
        if let Some(i) = self.order.iter().position(|k| k == key)
            && let Some(k) = self.order.remove(i)
        {
            self.order.push_back(k);
        }
        image
    }

    /// True once per key that is neither cached nor being loaded; marks it as loading.
    pub fn begin_load(&mut self, key: &CoverKey) -> bool {
        !self.entries.contains_key(key) && self.pending.insert(key.clone())
    }

    pub fn finish_load(&mut self, key: CoverKey, image: Option<Arc<Image>>) {
        self.pending.remove(&key);
        if self.entries.insert(key.clone(), image).is_none() {
            self.order.push_back(key);
        }
        while self.entries.len() > CAPACITY {
            match self.order.pop_front() {
                Some(old) => {
                    self.entries.remove(&old);
                }
                None => break,
            }
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Blocking: reads the embedded picture of `path` and wraps it for GPUI. Call off the UI thread.
pub fn load_image(path: &Path) -> Option<Arc<Image>> {
    let cover = cover::load(path)?;
    let format = ImageFormat::from_mime_type(&cover.mime).or_else(|| sniff(&cover.bytes))?;
    Some(Arc::new(Image::from_bytes(format, cover.bytes)))
}

/// Format from magic bytes, for files whose tag carries a wrong or missing mime type.
fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: usize) -> CoverKey {
        (String::new(), n.to_string())
    }

    #[test]
    fn evicts_least_recently_used_beyond_capacity() {
        let mut c = CoverCache::default();
        for n in 0..CAPACITY {
            assert!(c.begin_load(&key(n)));
            c.finish_load(key(n), None);
        }
        c.get(&key(0));
        c.finish_load(key(CAPACITY), None);
        assert_eq!(c.len(), CAPACITY);
        assert!(c.entries.contains_key(&key(0)));
        assert!(!c.entries.contains_key(&key(1)));
    }

    #[test]
    fn begin_load_is_once_per_key() {
        let mut c = CoverCache::default();
        assert!(c.begin_load(&key(1)));
        assert!(!c.begin_load(&key(1)));
        c.finish_load(key(1), None);
        assert!(!c.begin_load(&key(1)));
    }

    #[test]
    fn mime_and_sniffing() {
        assert_eq!(
            ImageFormat::from_mime_type("image/jpeg"),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_mime_type("image/jpg"),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_mime_type("application/octet-stream"),
            None
        );
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg));
        assert_eq!(sniff(b"nope"), None);
    }
}
