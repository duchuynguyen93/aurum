//! Gapless transitions: no inserted silence, no lost or extra frames at the join.

mod common;

use aurum_audio::{AudioEngine, Event};
use common::{fixture, longest_silence, render};

const RATE: u32 = 44_100;
/// 2 ms at 44.1 kHz.
const MAX_SILENCE_FRAMES: usize = 88;
/// +-50 ms around the join.
const WINDOW: usize = 2205;

/// Every fixture is exactly 2 s at 44.1 kHz.
const NOMINAL_FRAMES: usize = 2 * RATE as usize;

/// Renders `a` then `b` gaplessly and checks the join. With `exact_length` the total must be
/// exactly the nominal length of both files, i.e. encoder delay/padding were removed.
fn check_pair(a: &str, b: &str, exact_length: bool) {
    let (pa, pb) = (fixture(a), fixture(b));
    let alone_a = render(RATE, 1, 1, |e| e.load(&pa, None)).frames();
    let alone_b = render(RATE, 1, 1, |e| e.load(&pb, None)).frames();

    let joined = render(RATE, 1, 2, |e| {
        e.load(&pa, None);
        e.queue_next(&pb);
    });
    let silence = longest_silence(&joined.samples, alone_a - WINDOW, alone_a + WINDOW);
    eprintln!(
        "{a} -> {b}: alone {alone_a} + {alone_b}, joined {} (nominal {}), longest silent run at the join {silence} frames",
        joined.frames(),
        2 * NOMINAL_FRAMES
    );

    // The join itself must neither drop nor add a frame relative to the two files on their own.
    assert_eq!(
        joined.frames(),
        alone_a + alone_b,
        "frame count at the join"
    );
    assert!(
        silence < MAX_SILENCE_FRAMES,
        "{silence} silent frames at the join (limit {MAX_SILENCE_FRAMES})"
    );
    if exact_length {
        assert_eq!(alone_a, NOMINAL_FRAMES, "{a}: delay/padding not trimmed");
        assert_eq!(alone_b, NOMINAL_FRAMES, "{b}: delay/padding not trimmed");
    }

    // Event order: Loaded, TrackEnded(a), TrackStarted(b), TrackEnded(b).
    let kinds: Vec<&str> = joined
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Loaded { .. } => Some("loaded"),
            Event::TrackEnded { .. } => Some("ended"),
            Event::TrackStarted { .. } => Some("started"),
            Event::Error(m) => panic!("engine error: {m}"),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["loaded", "ended", "started", "ended"]);
}

#[test]
fn flac_to_flac() {
    check_pair("sine440.flac", "sine880.flac", true);
}

#[test]
fn mp3_to_mp3() {
    // symphonia reads the LAME/Xing header and trims encoder delay and padding.
    check_pair("sine440.mp3", "sine880.mp3", true);
}

/// The join is clean, but see `flac_to_m4a_exact_length` for the delay/padding caveat.
#[test]
fn flac_to_m4a_no_gap() {
    check_pair("sine440.flac", "sine880.m4a", false);
}

/// symphonia-format-isomp4 0.6.1 parses the `edts`/`elst` atom but never applies it (no
/// delay/padding on the track, no trim on packets, no iTunSMPB), so AAC priming and padding stay
/// in the output: the M4A decodes to 90112 frames instead of 88200.
#[test]
#[ignore = "symphonia-format-isomp4 0.6.1 ignores the edit list: M4A keeps 1912 extra priming/padding frames"]
fn flac_to_m4a_exact_length() {
    check_pair("sine440.flac", "sine880.m4a", true);
}

/// Different source rates must still join cleanly because everything is resampled to the sink.
#[test]
fn flac_44k_to_flac_48k_is_gapless_on_a_44k_sink() {
    let (pa, pb) = (fixture("sine440.flac"), fixture("sine880_48k.flac"));
    let joined = render(RATE, 1, 2, |e| {
        e.load(&pa, None);
        e.queue_next(&pb);
    });
    let expected = 2 * NOMINAL_FRAMES;
    let diff = joined.frames().abs_diff(expected);
    let silence = longest_silence(
        &joined.samples,
        NOMINAL_FRAMES - WINDOW,
        NOMINAL_FRAMES + WINDOW,
    );
    eprintln!(
        "44k->48k join: {} frames (expected {expected}), silent run {silence}",
        joined.frames()
    );
    assert!(diff <= 2, "frame count off by {diff}");
    assert!(silence < MAX_SILENCE_FRAMES);
}
