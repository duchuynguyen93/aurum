use std::path::Path;

use aurum_core::library::cover;

fn fixture(dir: &Path) -> std::path::PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sine440.wav");
    let dst = dir.join("track.wav");
    std::fs::copy(src, &dst).unwrap();
    dst
}

#[test]
fn sidecar_is_used_when_no_embedded_art() {
    let dir = tempfile::tempdir().unwrap();
    let track = fixture(dir.path());
    let png = b"\x89PNG\r\n\x1a\nfake".to_vec();
    std::fs::write(dir.path().join("Folder.PNG"), &png).unwrap();
    let c = cover::load(&track).unwrap();
    assert_eq!(c.mime, "image/png");
    assert_eq!(c.bytes, png);
}

#[test]
fn sidecar_stem_order_wins() {
    let dir = tempfile::tempdir().unwrap();
    let track = fixture(dir.path());
    std::fs::write(dir.path().join("front.jpg"), b"front").unwrap();
    std::fs::write(dir.path().join("Cover.webp"), b"cover").unwrap();
    let c = cover::load(&track).unwrap();
    assert_eq!(c.mime, "image/webp");
    assert_eq!(c.bytes, b"cover");
}

#[test]
fn no_cover_yields_none() {
    let dir = tempfile::tempdir().unwrap();
    let track = fixture(dir.path());
    std::fs::write(dir.path().join("notes.png"), b"x").unwrap();
    assert!(cover::load(&track).is_none());
}
