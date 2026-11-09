//! Plays files back to back (gapless) through the default output device.
//!
//! ```text
//! cargo run -p aurum-audio --example play -- a.flac b.mp3
//! ```

use std::path::PathBuf;
use std::time::Duration;

use aurum_audio::{AudioEngine, CpalEngine, CpalSink, Event, Sink};

fn main() {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if files.is_empty() {
        eprintln!("usage: play <file> [file...]");
        std::process::exit(2);
    }

    let sink = match CpalSink::new() {
        Ok(sink) => sink,
        Err(e) => {
            eprintln!("cannot open output: {e}");
            std::process::exit(1);
        }
    };
    let spec = sink.spec();
    println!(
        "output: {} Hz, {} ch, device format {}",
        spec.sample_rate,
        spec.channels,
        sink.sample_format()
    );
    let mut engine = CpalEngine::with_sink(sink).expect("start engine");
    let events = engine.events();

    // Queue the second file only after the first is loaded, then the rest one by one as each
    // TrackStarted arrives, so the engine always has exactly one track preloaded.
    engine.load(&files[0], None);
    let mut next_index = 1;
    if let Some(path) = files.get(next_index) {
        engine.queue_next(path);
        next_index += 1;
    }
    engine.play();

    let mut ended = 0;
    let mut underruns = 0;
    while ended < files.len() {
        match events.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Position(p)) => println!("position {:.1}s", p.as_secs_f64()),
            Ok(Event::TrackEnded { path }) => {
                println!("TrackEnded {}", path.display());
                ended += 1;
            }
            Ok(Event::TrackStarted { path }) => {
                println!("TrackStarted {}", path.display());
                if let Some(path) = files.get(next_index) {
                    engine.queue_next(path);
                    next_index += 1;
                }
            }
            Ok(Event::Underrun) => {
                underruns += 1;
                println!("Underrun");
            }
            Ok(event @ Event::Loaded { .. }) => println!("{event:?}"),
            Ok(Event::Error(msg)) => {
                println!("Error: {msg}");
                // A failed load or queue never produces a TrackEnded; do not wait forever.
                ended += 1;
            }
            Err(_) => {
                eprintln!("timed out waiting for events");
                break;
            }
        }
    }
    println!("done, {underruns} underrun event(s)");
}
