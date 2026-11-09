mod common;

use std::time::Duration;

use aurum_audio::{AudioEngine, Engine, Event, WavSink};
use common::{fixture, render};

#[test]
fn bad_files_report_errors_and_the_thread_survives() {
    let garbage = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("garbage.flac");
    std::fs::write(&garbage, b"this is not audio at all, not even close to it").unwrap();
    let missing = fixture("does-not-exist.flac");
    let good = fixture("sine440.flac");

    let r = render(44_100, 1, 1, |e| {
        e.load(&missing, None);
        e.load(&garbage, None);
        e.load(&good, None);
    });
    let errors = r
        .events
        .iter()
        .filter(|e| matches!(e, Event::Error(_)))
        .count();
    assert_eq!(errors, 2, "events: {:?}", r.events);
    assert_eq!(r.frames(), 2 * 44_100);
}

#[test]
fn shutdown_without_loading_is_clean() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let engine = Engine::with_sink(WavSink::new(dir.join("empty.wav"), 44_100, 2)).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let mut sink = engine.shutdown();
    sink.finish().unwrap();
}
