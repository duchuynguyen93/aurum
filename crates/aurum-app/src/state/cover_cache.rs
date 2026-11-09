//! Album cover cache: `(artist, album)` to pre-scaled GPUI images, bounded LRU.
//!
//! Each entry holds two images made once on a background thread: a bounded-size thumbnail for
//! the grid, the bar and the album header, and a tiny pre-blurred copy used as ambient
//! background. Full-size embedded pictures are never handed to GPUI, so 100 albums do not
//! mean 100 multi-megapixel textures.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use aurum_core::Track;
use aurum_core::library::cover;
use gpui_kit::RenderImage;
use image::imageops::{self, FilterType};
use image::{Frame, RgbaImage};

pub const CAPACITY: usize = 200;
/// Longest side of the thumbnail in pixels (enough for a 240pt cover on a 2x screen).
const THUMB_SIDE: u32 = 400;
/// Side of the square image the blur is made from.
const BLUR_SIDE: u32 = 64;
const BLUR_SIGMA: f32 = 4.0;

pub type CoverKey = (String, String);

/// Decoded cover, ready to draw.
#[derive(Clone)]
pub struct CoverArt {
    pub thumb: Arc<RenderImage>,
    /// Small, already blurred; stretch it over a large area for a soft backdrop.
    pub blur: Arc<RenderImage>,
}

/// What the cache knows about a key.
pub enum Lookup {
    Ready(CoverArt),
    /// The album has no usable cover.
    Missing,
    /// Not loaded yet (queued, loading or evicted).
    Loading,
}

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
    entries: HashMap<CoverKey, Option<CoverArt>>,
    /// Least recently used first.
    order: VecDeque<CoverKey>,
    pending: HashSet<CoverKey>,
}

impl CoverCache {
    pub fn lookup(&mut self, key: &CoverKey) -> Lookup {
        match self.entries.get(key) {
            None => Lookup::Loading,
            Some(None) => Lookup::Missing,
            Some(Some(art)) => {
                let art = art.clone();
                if let Some(i) = self.order.iter().position(|k| k == key)
                    && let Some(k) = self.order.remove(i)
                {
                    self.order.push_back(k);
                }
                Lookup::Ready(art)
            }
        }
    }

    pub fn get(&mut self, key: &CoverKey) -> Option<CoverArt> {
        match self.lookup(key) {
            Lookup::Ready(art) => Some(art),
            _ => None,
        }
    }

    /// True once per key that is neither cached nor being loaded; marks it as loading.
    pub fn begin_load(&mut self, key: &CoverKey) -> bool {
        !self.entries.contains_key(key) && self.pending.insert(key.clone())
    }

    pub fn finish_load(&mut self, key: CoverKey, art: Option<CoverArt>) {
        self.pending.remove(&key);
        if self.entries.insert(key.clone(), art).is_none() {
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

/// Blocking: reads the embedded picture of `path` and scales it. Call off the UI thread.
pub fn load_art(path: &Path) -> Option<CoverArt> {
    let cover = cover::load(path)?;
    let art = make_art(&cover.bytes);
    if art.is_none() {
        log::warn!(
            "cannot decode cover of {} ({}, {} bytes)",
            path.display(),
            cover.mime,
            cover.bytes.len()
        );
    }
    art
}

/// Decodes (format is guessed from the bytes, the tag's mime type is often wrong) and builds
/// the thumbnail and the blurred copy. JPEG and PNG only.
pub fn make_art(bytes: &[u8]) -> Option<CoverArt> {
    let decoded = image::load_from_memory(bytes).ok()?;
    let thumb = if decoded.width().max(decoded.height()) > THUMB_SIDE {
        decoded.resize(THUMB_SIDE, THUMB_SIDE, FilterType::Triangle)
    } else {
        decoded
    };
    let thumb = thumb.into_rgba8();
    let small = imageops::resize(&thumb, BLUR_SIDE, BLUR_SIDE, FilterType::Triangle);
    let blur = imageops::blur(&small, BLUR_SIGMA);
    Some(CoverArt {
        thumb: render_image(thumb),
        blur: render_image(blur),
    })
}

/// GPUI wants BGRA.
fn render_image(mut rgba: RgbaImage) -> Arc<RenderImage> {
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![Frame::new(rgba)]))
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
        c.finish_load(key(CAPACITY), None);
        assert_eq!(c.len(), CAPACITY);
        assert!(!c.entries.contains_key(&key(0)));
        assert!(c.entries.contains_key(&key(1)));
    }

    #[test]
    fn begin_load_is_once_per_key() {
        let mut c = CoverCache::default();
        assert!(c.begin_load(&key(1)));
        assert!(!c.begin_load(&key(1)));
        c.finish_load(key(1), None);
        assert!(!c.begin_load(&key(1)));
        assert!(matches!(c.lookup(&key(1)), Lookup::Missing));
        assert!(matches!(c.lookup(&key(2)), Lookup::Loading));
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba([200, 100, 50, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn art_is_scaled_down_and_garbage_is_rejected() {
        let art = make_art(&png(1000, 800)).expect("png decodes");
        // The decoded frame is bounded by THUMB_SIDE, not the 1000px original.
        let bytes = art.thumb.as_bytes(0).unwrap().len() as u32;
        assert!(bytes <= THUMB_SIDE * THUMB_SIDE * 4);
        assert_eq!(
            art.blur.as_bytes(0).unwrap().len() as u32,
            BLUR_SIDE * BLUR_SIDE * 4
        );
        assert!(make_art(b"not an image").is_none());
    }
}
