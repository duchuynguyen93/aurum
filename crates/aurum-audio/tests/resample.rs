mod common;

use aurum_audio::AudioEngine;
use common::{fixture, render};

#[test]
fn flac_48k_through_a_44k_sink() {
    let path = fixture("sine880_48k.flac");
    let r = render(44_100, 1, 1, |e| e.load(&path, None));
    let expected = 2 * 44_100;
    let diff = r.frames().abs_diff(expected);
    eprintln!(
        "48k->44.1k: {} frames, expected {expected}, off by {diff}",
        r.frames()
    );
    assert!(diff * 100 <= expected, "off by {diff} frames");

    // The 880 Hz tone must survive: ~1760 cycles in 2 s -> ~3520 sign changes.
    let crossings = r
        .samples
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    eprintln!("zero crossings: {crossings}");
    assert!((3480..=3560).contains(&crossings), "crossings {crossings}");
}

#[test]
fn flac_44k_through_a_48k_sink() {
    let path = fixture("sine440.flac");
    let r = render(48_000, 1, 1, |e| e.load(&path, None));
    let expected = 2 * 48_000;
    assert!(
        r.frames().abs_diff(expected) * 100 <= expected,
        "{} frames",
        r.frames()
    );
}

#[test]
fn mono_file_is_duplicated_on_a_stereo_sink() {
    let path = fixture("sine440.flac");
    let r = render(44_100, 2, 1, |e| e.load(&path, None));
    assert_eq!(r.frames(), 2 * 44_100);
    assert!((0..r.frames()).all(|i| r.samples[2 * i] == r.samples[2 * i + 1]));
}
