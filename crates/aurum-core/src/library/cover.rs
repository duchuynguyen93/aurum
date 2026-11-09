use std::path::Path;

use lofty::file::TaggedFileExt;

/// Embedded cover art.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Reads the first embedded picture of `path`, if any. Covers are deliberately not stored in
/// the database; callers cache them per album.
pub fn load(path: &Path) -> Option<Cover> {
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
