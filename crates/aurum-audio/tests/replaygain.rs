mod common;

use aurum_audio::{AudioEngine, ReplayGain};
use common::{fixture, render, rms};

#[test]
fn minus_six_db_halves_rms() {
    let path = fixture("sine440.flac");
    let plain = render(44_100, 1, 1, |e| e.load(&path, None));
    let quiet = render(44_100, 1, 1, |e| {
        e.load(&path, Some(ReplayGain::from(-6.02)))
    });
    let ratio = rms(&quiet.samples) / rms(&plain.samples);
    eprintln!(
        "rms plain {:.5}, rms -6.02 dB {:.5}, ratio {ratio:.4}",
        rms(&plain.samples),
        rms(&quiet.samples)
    );
    assert!((ratio - 0.5).abs() < 0.5 * 0.02, "ratio {ratio}");
}

#[test]
fn peak_prevents_boost_from_clipping() {
    // The fixture peaks near 0.125; claiming a peak of 1.0 limits +12 dB to unity gain.
    let path = fixture("sine440.flac");
    let plain = render(44_100, 1, 1, |e| e.load(&path, None));
    let boosted = render(44_100, 1, 1, |e| {
        e.load(
            &path,
            Some(ReplayGain {
                gain_db: 12.0,
                peak: Some(1.0),
            }),
        );
    });
    let ratio = rms(&boosted.samples) / rms(&plain.samples);
    assert!((ratio - 1.0).abs() < 0.02, "ratio {ratio}");
}

#[test]
fn volume_uses_square_curve() {
    let path = fixture("sine440.flac");
    let plain = render(44_100, 1, 1, |e| e.load(&path, None));
    let half = render(44_100, 1, 1, |e| {
        e.set_volume(0.5);
        e.load(&path, None);
    });
    let ratio = rms(&half.samples) / rms(&plain.samples);
    assert!((ratio - 0.25).abs() < 0.25 * 0.02, "ratio {ratio}");
}
