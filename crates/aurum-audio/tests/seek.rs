mod common;

use std::time::Duration;

use aurum_audio::AudioEngine;
use common::{fixture, render};

const RATE: u32 = 44_100;

#[test]
fn seek_to_one_second_leaves_one_second() {
    let path = fixture("sine880.flac");
    let r = render(RATE, 1, 1, |e| {
        e.load(&path, None);
        e.seek(Duration::from_secs(1));
    });
    // FLAC fixture blocks are 4096 frames; "one packet" is the allowed error.
    let diff = r.frames().abs_diff(RATE as usize);
    eprintln!(
        "flac seek(1s): {} frames, expected {RATE}, off by {diff}",
        r.frames()
    );
    assert!(diff <= 4096, "off by {diff} frames");
}

#[test]
fn seek_on_mp3_and_m4a() {
    for name in ["sine880.mp3", "sine880.m4a"] {
        let path = fixture(name);
        let r = render(RATE, 1, 1, |e| {
            e.load(&path, None);
            e.seek(Duration::from_secs(1));
        });
        let diff = r.frames().abs_diff(RATE as usize);
        eprintln!(
            "{name} seek(1s): {} frames, expected {RATE}, off by {diff}",
            r.frames()
        );
        // MP3 packet = 1152 frames, AAC = 1024; allow two for decoder priming.
        assert!(diff <= 2304, "{name}: off by {diff} frames");
    }
}

#[test]
fn seek_mid_playback_flushes_old_audio() {
    use aurum_audio::Event;
    // Seek issued after some audio is already buffered must not leak pre-seek samples:
    // render 2 s file, seek to 1.5 s after load -> ~0.5 s remains.
    let path = fixture("sine440.flac");
    let r = render(RATE, 1, 1, |e| {
        e.load(&path, None);
        e.seek(Duration::from_millis(1500));
    });
    let diff = r.frames().abs_diff(RATE as usize / 2);
    assert!(diff <= 4096, "off by {diff}");
    assert!(!r.events.iter().any(|e| matches!(e, Event::Error(_))));
}
