use std::fs;
use std::path::{Path, PathBuf};

use aurum_core::Track;
use aurum_core::library::scan::parse_replay_gain;
use aurum_core::library::{Library, ScanEvent, ScanSummary, Scanner, cover};
use crossbeam_channel::unbounded;
use tempfile::TempDir;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Copies the audio fixtures into a fresh temp dir so tests never touch the originals.
fn fixture_copy() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for entry in fs::read_dir(fixtures()).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|e| e != "sh") {
            fs::copy(&p, dir.path().join(p.file_name().unwrap())).unwrap();
        }
    }
    dir
}

fn scan(lib: &mut Library, root: &Path) -> (ScanSummary, Vec<ScanEvent>) {
    let (tx, rx) = unbounded();
    let summary = Scanner::scan_blocking(lib, &[root.to_path_buf()], &tx);
    drop(tx);
    (summary, rx.iter().collect())
}

fn by_name<'a>(tracks: &'a [Track], name: &str) -> &'a Track {
    tracks
        .iter()
        .find(|t| t.path.file_name().is_some_and(|n| n == name))
        .unwrap_or_else(|| panic!("{name} not in library"))
}

#[test]
fn scan_reads_all_fixtures_with_correct_tags() {
    let dir = fixture_copy();
    let mut lib = Library::open_in_memory().unwrap();
    let (summary, events) = scan(&mut lib, dir.path());

    assert_eq!(summary.scanned, 8);
    assert_eq!(summary.added, 8);
    assert_eq!(summary.errors, 0, "{events:?}");
    assert_eq!(lib.track_count().unwrap(), 8);
    assert!(matches!(
        events.first(),
        Some(ScanEvent::Started { total_hint: 8 })
    ));
    assert!(matches!(events.last(), Some(ScanEvent::Finished { .. })));

    let tracks = lib.all_tracks().unwrap();

    let flac = by_name(&tracks, "sine440.flac");
    assert_eq!(flac.title, "Sine 440 FLAC");
    assert_eq!(flac.artist.as_deref(), Some("Aurum Test"));
    assert_eq!(flac.album_artist.as_deref(), Some("Aurum Test"));
    assert_eq!(flac.album.as_deref(), Some("Tones"));
    assert_eq!(flac.track_no, Some(1));
    assert_eq!(flac.year, Some(2020));
    assert_eq!(flac.genre.as_deref(), Some("Test"));
    assert_eq!(flac.codec, "flac");
    assert_eq!(flac.sample_rate, 44100);
    assert_eq!(flac.channels, 1);
    assert!(
        (1900..=2100).contains(&flac.duration_ms),
        "{}",
        flac.duration_ms
    );
    assert_eq!(flac.replay_gain.track_gain, Some(-6.02));
    assert_eq!(flac.replay_gain.track_peak, Some(0.5));
    assert_eq!(flac.replay_gain.album_gain, None);

    let mp3 = by_name(&tracks, "sine880.mp3");
    assert_eq!(mp3.codec, "mp3");
    assert_eq!(mp3.track_no, Some(2));
    assert_eq!(mp3.replay_gain.track_gain, Some(-3.0));

    let m4a = by_name(&tracks, "sine440.m4a");
    assert_eq!(m4a.codec, "m4a");
    assert_eq!(m4a.artist.as_deref(), Some("Guest Artist"));
    assert_eq!(m4a.album_artist.as_deref(), Some("Aurum Test"));
    assert_eq!(m4a.disc_no, Some(2));

    // Untagged WAV: title falls back to the file stem, no album.
    let wav = by_name(&tracks, "sine440.wav");
    assert_eq!(wav.title, "sine440");
    assert_eq!(wav.artist, None);
    assert_eq!(wav.album, None);
    assert_eq!(wav.codec, "wav");
    assert!(
        (900..=1100).contains(&wav.duration_ms),
        "{}",
        wav.duration_ms
    );
}

#[test]
fn second_scan_changes_nothing_and_modified_file_is_updated() {
    let dir = fixture_copy();
    let mut lib = Library::open_in_memory().unwrap();
    scan(&mut lib, dir.path());

    let (again, _) = scan(&mut lib, dir.path());
    assert_eq!((again.added, again.updated, again.removed), (0, 0, 0));
    assert_eq!(again.unchanged, 8);

    // Bump mtime of one file: it must be re-read as an update, not an add.
    let f = fs::File::options()
        .write(true)
        .open(dir.path().join("sine440.flac"))
        .unwrap();
    f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60))
        .unwrap();
    drop(f);
    let (third, _) = scan(&mut lib, dir.path());
    assert_eq!((third.added, third.updated, third.unchanged), (0, 1, 7));
    assert_eq!(lib.track_count().unwrap(), 8);
}

#[test]
fn deleted_file_is_removed_from_database() {
    let dir = fixture_copy();
    let mut lib = Library::open_in_memory().unwrap();
    scan(&mut lib, dir.path());
    fs::remove_file(dir.path().join("sine880.mp3")).unwrap();

    let (summary, _) = scan(&mut lib, dir.path());
    assert_eq!(summary.removed, 1);
    assert_eq!(lib.track_count().unwrap(), 7);
    assert!(
        lib.all_tracks()
            .unwrap()
            .iter()
            .all(|t| !t.path.ends_with("sine880.mp3"))
    );
}

#[test]
fn missing_root_does_not_wipe_library() {
    let dir = fixture_copy();
    let mut lib = Library::open_in_memory().unwrap();
    scan(&mut lib, dir.path());

    let gone = dir.path().join("does-not-exist");
    let (tx, rx) = unbounded();
    let summary = Scanner::scan_blocking(&mut lib, &[gone], &tx);
    drop(tx);
    assert_eq!(summary.removed, 0);
    assert_eq!(summary.errors, 1);
    assert!(rx.iter().any(|e| matches!(e, ScanEvent::Error { .. })));
    assert_eq!(lib.track_count().unwrap(), 8);
}

#[test]
fn corrupt_file_reports_error_and_scan_continues() {
    let dir = fixture_copy();
    fs::write(dir.path().join("broken.flac"), b"this is not audio").unwrap();
    fs::write(dir.path().join("empty.mp3"), b"").unwrap();
    let mut lib = Library::open_in_memory().unwrap();
    let (summary, events) = scan(&mut lib, dir.path());

    assert_eq!(summary.errors, 2);
    assert_eq!(lib.track_count().unwrap(), 8);
    let errors = events
        .iter()
        .filter(|e| matches!(e, ScanEvent::Error { .. }))
        .count();
    assert_eq!(errors, 2);
}

#[test]
fn album_grouping_falls_back_to_artist_when_album_artist_missing() {
    let dir = fixture_copy();
    let mut lib = Library::open_in_memory().unwrap();
    scan(&mut lib, dir.path());

    let albums = lib.albums().unwrap();
    // "Beeps" has no album_artist tag, so it is grouped under its artist.
    let beeps = albums.iter().find(|a| a.name == "Beeps").unwrap();
    assert_eq!(beeps.artist.as_deref(), Some("Beep Band"));
    assert_eq!(beeps.track_count, 2);
    assert_eq!(beeps.year, Some(2021));
    // "Tones" mixes FLAC (artist Aurum Test) and M4A (artist Guest Artist, album_artist
    // Aurum Test): one album thanks to album_artist, not split by track artist.
    let tones: Vec<_> = albums.iter().filter(|a| a.name == "Tones").collect();
    assert_eq!(tones.len(), 1);
    assert_eq!(tones[0].track_count, 4);
    // Untagged WAVs have no album and are not listed.
    assert_eq!(albums.len(), 2);

    let tracks = lib.tracks_of_album(Some("Aurum Test"), "Tones").unwrap();
    let order: Vec<_> = tracks
        .iter()
        .map(|t| (t.disc_no, t.track_no, t.codec.as_str()))
        .collect();
    // disc NULL sorts first, then disc 2.
    assert_eq!(
        order,
        vec![
            (None, Some(1), "flac"),
            (None, Some(2), "flac"),
            (Some(2), Some(1), "m4a"),
            (Some(2), Some(2), "m4a"),
        ]
    );
    assert_eq!(
        lib.tracks_of_album(Some("Beep Band"), "Beeps")
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn search_matches_title_artist_and_album_case_insensitively() {
    let dir = fixture_copy();
    let mut lib = Library::open_in_memory().unwrap();
    scan(&mut lib, dir.path());

    assert_eq!(lib.search("sine 440").unwrap().len(), 3); // flac, mp3, m4a titles
    assert_eq!(lib.search("BEEP").unwrap().len(), 2); // artist + album of the MP3s
    assert_eq!(lib.search("tones").unwrap().len(), 4);
    assert!(lib.search("%").unwrap().is_empty(), "% must be literal");
    assert!(lib.search("nope").unwrap().is_empty());
}

#[test]
fn background_scan_persists_to_file_and_reader_connection_sees_it() {
    let dir = fixture_copy();
    let db_dir = tempfile::tempdir().unwrap();
    let db_path = db_dir.path().join("lib.db");

    let (tx, rx) = unbounded();
    let handle = Scanner::new(&db_path).scan(&[dir.path().to_path_buf()], tx);
    let events: Vec<_> = rx.iter().collect();
    handle.join().unwrap();
    assert!(matches!(events.last(), Some(ScanEvent::Finished { summary }) if summary.added == 8));

    let reader = Library::open(&db_path).unwrap();
    assert_eq!(reader.track_count().unwrap(), 8);
}

#[test]
fn cover_load_returns_none_without_picture() {
    assert!(cover::load(&fixtures().join("sine440.flac")).is_none());
    assert!(cover::load(&fixtures().join("nope.flac")).is_none());
}

#[test]
fn replay_gain_parsing() {
    assert_eq!(parse_replay_gain("-6.02 dB"), Some(-6.02));
    assert_eq!(parse_replay_gain("+1.5 dB"), Some(1.5));
    assert_eq!(parse_replay_gain("  -0.5dB "), Some(-0.5));
    assert_eq!(parse_replay_gain("3 db"), Some(3.0));
    assert_eq!(parse_replay_gain("0.988553"), Some(0.988553));
    assert_eq!(parse_replay_gain(""), None);
    assert_eq!(parse_replay_gain("dB"), None);
    assert_eq!(parse_replay_gain("abc dB"), None);
    assert_eq!(parse_replay_gain("NaN"), None);
    assert_eq!(parse_replay_gain("é"), None);
}
