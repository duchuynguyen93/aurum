//! Scans a folder into a temporary library and prints a short report.
//! Usage: cargo run -p aurum-core --example scan -- <folder>

use std::path::PathBuf;
use std::time::Instant;

use aurum_core::library::{Library, ScanEvent, Scanner};
use crossbeam_channel::unbounded;

fn main() {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: scan <folder>");
        std::process::exit(2);
    };
    let db_dir = tempfile::tempdir().expect("temp dir");
    let db_path = db_dir.path().join("library.db");

    let started = Instant::now();
    let (tx, rx) = unbounded();
    let handle = Scanner::new(&db_path).scan(&[root], tx);
    let mut errors = 0;
    for event in rx {
        match event {
            ScanEvent::Error { path, msg } => {
                errors += 1;
                eprintln!("error: {}: {msg}", path.display());
            }
            ScanEvent::Finished { .. } => break,
            _ => {}
        }
    }
    handle.join().expect("scan thread");
    let elapsed = started.elapsed();

    let lib = Library::open(&db_path).expect("open library");
    let albums = lib.albums().expect("albums");
    println!("tracks:  {}", lib.track_count().expect("count"));
    println!("albums:  {}", albums.len());
    println!("errors:  {errors}");
    println!("scan:    {elapsed:.2?}");
    for a in albums.iter().take(5) {
        println!(
            "  {} - {} ({}) [{} tracks]",
            a.artist.as_deref().unwrap_or("?"),
            a.name,
            a.year.map_or_else(|| "?".to_owned(), |y| y.to_string()),
            a.track_count
        );
    }
}
