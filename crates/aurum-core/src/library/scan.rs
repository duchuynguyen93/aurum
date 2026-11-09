use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::Sender;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey, Tag};

use super::Library;
use super::db::path_to_string;
use crate::{ReplayGain, Track};

/// File extensions (lowercase) the scanner picks up.
const EXTENSIONS: &[&str] = &[
    "flac", "mp3", "m4a", "aac", "ogg", "oga", "wav", "aif", "aiff",
];

/// Emit a `Progress` event every this many files.
const PROGRESS_EVERY: usize = 25;

/// Totals of one scan run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanSummary {
    /// Audio files found on disk (including unchanged ones and ones that failed to read).
    pub scanned: usize,
    pub added: usize,
    pub updated: usize,
    /// Files skipped because mtime and size matched the database.
    pub unchanged: usize,
    /// Rows deleted because the file is gone.
    pub removed: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    Started {
        total_hint: usize,
    },
    Progress {
        scanned: usize,
        added: usize,
        updated: usize,
    },
    Finished {
        summary: ScanSummary,
    },
    Error {
        path: PathBuf,
        msg: String,
    },
}

/// Walks music folders and mirrors them into a [`Library`].
///
/// After a scan the database mirrors exactly the audio files under the given roots: rows
/// outside the roots are removed too. If a root is missing or a directory cannot be read,
/// nothing is removed, so an unmounted drive does not wipe the library.
pub struct Scanner {
    db_path: PathBuf,
}

impl Scanner {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    /// Scans on a background thread with its own writer connection to the database.
    /// Events end with `Finished`, also when the database cannot be opened.
    pub fn scan(&self, roots: &[PathBuf], tx: Sender<ScanEvent>) -> JoinHandle<()> {
        let db_path = self.db_path.clone();
        let roots = roots.to_vec();
        thread::spawn(move || match Library::open(&db_path) {
            Ok(mut lib) => {
                Self::scan_blocking(&mut lib, &roots, &tx);
            }
            Err(e) => {
                let _ = tx.send(ScanEvent::Error {
                    path: db_path,
                    msg: e.to_string(),
                });
                let _ = tx.send(ScanEvent::Finished {
                    summary: ScanSummary::default(),
                });
            }
        })
    }

    /// Scans on the calling thread against an already open library.
    pub fn scan_blocking(
        lib: &mut Library,
        roots: &[PathBuf],
        tx: &Sender<ScanEvent>,
    ) -> ScanSummary {
        let mut summary = ScanSummary::default();
        let mut complete = true;
        let error = |path: &Path, msg: String| {
            let _ = tx.send(ScanEvent::Error {
                path: path.to_path_buf(),
                msg,
            });
        };

        let mut files: Vec<PathBuf> = Vec::new();
        for root in roots {
            if !root.is_dir() {
                error(root, "scan root is not a readable directory".into());
                summary.errors += 1;
                complete = false;
                continue;
            }
            for entry in jwalk::WalkDir::new(root) {
                match entry {
                    Ok(e) if e.file_type().is_file() && has_audio_extension(&e.path()) => {
                        files.push(e.path());
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let path = e.path().unwrap_or(root).to_path_buf();
                        error(&path, e.to_string());
                        summary.errors += 1;
                        complete = false;
                    }
                }
            }
        }
        let _ = tx.send(ScanEvent::Started {
            total_hint: files.len(),
        });

        let mut seen: HashSet<String> = HashSet::with_capacity(files.len());
        let result = lib.transaction(|lib| {
            for (i, path) in files.iter().enumerate() {
                seen.insert(path_to_string(path));
                summary.scanned += 1;
                match scan_file(lib, path) {
                    Ok(Outcome::Unchanged) => summary.unchanged += 1,
                    Ok(Outcome::Added) => summary.added += 1,
                    Ok(Outcome::Updated) => summary.updated += 1,
                    Err(msg) => {
                        error(path, msg);
                        summary.errors += 1;
                    }
                }
                if (i + 1) % PROGRESS_EVERY == 0 {
                    let _ = tx.send(ScanEvent::Progress {
                        scanned: summary.scanned,
                        added: summary.added,
                        updated: summary.updated,
                    });
                }
            }
            if complete {
                summary.removed = lib.remove_missing(&seen)?;
            }
            Ok(())
        });
        if let Err(e) = result {
            error(Path::new(""), format!("database error: {e}"));
            summary.errors += 1;
        }

        let _ = tx.send(ScanEvent::Progress {
            scanned: summary.scanned,
            added: summary.added,
            updated: summary.updated,
        });
        let _ = tx.send(ScanEvent::Finished { summary });
        summary
    }
}

enum Outcome {
    Unchanged,
    Added,
    Updated,
}

fn scan_file(lib: &Library, path: &Path) -> Result<Outcome, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = meta.modified().map(to_nanos).map_err(|e| e.to_string())?;
    let size = meta.len();
    let known = lib.stat(path).map_err(|e| e.to_string())?;
    if known == Some((mtime, size)) {
        return Ok(Outcome::Unchanged);
    }
    let track = read_track(path, mtime, size)?;
    lib.upsert(&track).map_err(|e| e.to_string())?;
    Ok(if known.is_some() {
        Outcome::Updated
    } else {
        Outcome::Added
    })
}

fn to_nanos(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i64,
        Err(e) => -(e.duration().as_nanos() as i64),
    }
}

fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Reads tags and audio properties of one file. Any parse failure is returned as a message.
fn read_track(path: &Path, mtime: i64, size: u64) -> Result<Track, String> {
    let tagged = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    let props = tagged.properties();
    let tag: Option<&Tag> = tagged.primary_tag().or_else(|| tagged.first_tag());

    let clean = |s: &str| Some(s.trim().to_owned()).filter(|s| !s.is_empty());
    let text = |s: Option<Cow<'_, str>>| s.and_then(|s| clean(&s));
    let rg = |key: ItemKey| {
        tag.and_then(|t| t.get_string(key))
            .and_then(parse_replay_gain)
    };

    let artist = tag.and_then(|t| text(t.artist()));
    let album_artist = tag
        .and_then(|t| t.get_string(ItemKey::AlbumArtist))
        .and_then(clean)
        .or_else(|| artist.clone());
    let title = tag
        .and_then(|t| text(t.title()))
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();

    Ok(Track {
        id: 0,
        path: path.to_path_buf(),
        title,
        artist,
        album_artist,
        album: tag.and_then(|t| text(t.album())),
        track_no: tag.and_then(|t| t.track()),
        disc_no: tag.and_then(|t| t.disk()),
        year: tag.and_then(|t| t.date()).map(|d| i32::from(d.year)),
        genre: tag.and_then(|t| text(t.genre())),
        duration_ms: u64::try_from(props.duration().as_millis()).unwrap_or(u64::MAX),
        sample_rate: props.sample_rate().unwrap_or(0),
        channels: props.channels().unwrap_or(0),
        codec: codec_name(tagged.file_type()),
        replay_gain: ReplayGain {
            track_gain: rg(ItemKey::ReplayGainTrackGain),
            track_peak: rg(ItemKey::ReplayGainTrackPeak),
            album_gain: rg(ItemKey::ReplayGainAlbumGain),
            album_peak: rg(ItemKey::ReplayGainAlbumPeak),
        },
        mtime,
        size,
    })
}

fn codec_name(ft: FileType) -> String {
    match ft {
        FileType::Flac => "flac".into(),
        FileType::Mpeg => "mp3".into(),
        FileType::Mp4 => "m4a".into(),
        FileType::Aac => "aac".into(),
        FileType::Vorbis => "vorbis".into(),
        FileType::Wav => "wav".into(),
        FileType::Aiff => "aiff".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Parses a ReplayGain value such as `"-6.02 dB"`, `"+1.5 dB"` or a bare peak like `"0.5"`.
/// Returns `None` for anything that is not a finite number.
pub fn parse_replay_gain(s: &str) -> Option<f32> {
    let s = s.trim();
    let num = match s.len().checked_sub(2).and_then(|i| s.get(i..)) {
        Some(suffix) if suffix.eq_ignore_ascii_case("db") => s[..s.len() - 2].trim_end(),
        _ => s,
    };
    num.parse::<f32>().ok().filter(|v| v.is_finite())
}
