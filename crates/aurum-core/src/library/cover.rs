use std::path::Path;

use lofty::file::TaggedFileExt;

/// Embedded cover art.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub mime: String,
    pub bytes: Vec<u8>,
}

const SIDECAR_STEMS: [&str; 4] = ["cover", "folder", "front", "album"];
const SIDECAR_EXTS: [(&str, &str); 4] = [
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
];

/// Reads the first embedded picture of `path`; without one, falls back to a sidecar image
/// (`cover`, `folder`, `front`, `album` + jpg/jpeg/png/webp, case-insensitive) in the same
/// directory. Covers are deliberately not stored in the database; callers cache them per album.
pub fn load(path: &Path) -> Option<Cover> {
    embedded(path).or_else(|| sidecar(path))
}

fn embedded(path: &Path) -> Option<Cover> {
    let tagged = lofty::read_from_path(path).ok()?;
    let picture = tagged.tags().iter().find_map(|t| t.pictures().first())?;
    Some(Cover {
        mime: picture
            .mime_type()
            .map_or("application/octet-stream", |m| m.as_str())
            .to_owned(),
        bytes: picture.data().to_vec(),
    })
}

/// Reads the directory once and picks the best match: stem order first, then extension order.
fn sidecar(path: &Path) -> Option<Cover> {
    let dir = path.parent()?;
    let mut best: Option<(usize, usize, std::path::PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        let (Some(stem), Some(ext)) = (p.file_stem(), p.extension()) else {
            continue;
        };
        let (stem, ext) = (
            stem.to_string_lossy().to_lowercase(),
            ext.to_string_lossy().to_lowercase(),
        );
        let Some(si) = SIDECAR_STEMS.iter().position(|s| *s == stem) else {
            continue;
        };
        let Some(ei) = SIDECAR_EXTS.iter().position(|(e, _)| *e == ext) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(bs, be, _)| (si, ei) < (*bs, *be))
        {
            best = Some((si, ei, p));
        }
    }
    let (_, ei, p) = best?;
    let bytes = std::fs::read(p).ok()?;
    Some(Cover {
        mime: SIDECAR_EXTS[ei].1.to_owned(),
        bytes,
    })
}
