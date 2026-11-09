use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::LibraryError;
use crate::{ReplayGain, Track};

const SCHEMA_VERSION: i32 = 1;

const MIGRATION_V1: &str = "
CREATE TABLE tracks (
    id            INTEGER PRIMARY KEY,
    path          TEXT NOT NULL UNIQUE,
    title         TEXT NOT NULL,
    artist        TEXT,
    album_artist  TEXT,
    album         TEXT,
    track_no      INTEGER,
    disc_no       INTEGER,
    year          INTEGER,
    genre         TEXT,
    duration_ms   INTEGER NOT NULL,
    sample_rate   INTEGER NOT NULL,
    channels      INTEGER NOT NULL,
    codec         TEXT NOT NULL,
    rg_track_gain REAL,
    rg_track_peak REAL,
    rg_album_gain REAL,
    rg_album_peak REAL,
    mtime         INTEGER NOT NULL,
    size          INTEGER NOT NULL
);
CREATE INDEX idx_tracks_album ON tracks(album_artist, album);
CREATE INDEX idx_tracks_artist ON tracks(artist);
";

const TRACK_COLUMNS: &str = "id, path, title, artist, album_artist, album, track_no, disc_no, \
    year, genre, duration_ms, sample_rate, channels, codec, \
    rg_track_gain, rg_track_peak, rg_album_gain, rg_album_peak, mtime, size";

/// One album, grouped by (album artist, album name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Album {
    pub name: String,
    pub artist: Option<String>,
    pub year: Option<i32>,
    pub track_count: u32,
    /// Path of one track of the album (lexicographically first), used to load cover art.
    pub first_track_path: PathBuf,
}

/// SQLite-backed library. One `Library` wraps one connection: the scanner thread owns the
/// writer, readers open their own connection on the same file (WAL allows concurrent reads).
///
/// Paths are stored as `String` via `to_string_lossy`; non-UTF-8 paths do not round-trip.
pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        // The pragma returns the resulting mode as a row, so it must go through a query.
        conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0))?;
        Self::init(conn)
    }

    /// In-memory database, mostly for tests.
    pub fn open_in_memory() -> Result<Self, LibraryError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, LibraryError> {
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Inserts the track, or updates the existing row with the same path. The `id` field of
    /// `track` is ignored.
    pub fn upsert(&self, t: &Track) -> Result<(), LibraryError> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO tracks (path, title, artist, album_artist, album, track_no, disc_no, \
                year, genre, duration_ms, sample_rate, channels, codec, \
                rg_track_gain, rg_track_peak, rg_album_gain, rg_album_peak, mtime, size) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19) \
             ON CONFLICT(path) DO UPDATE SET \
                title = excluded.title, artist = excluded.artist, \
                album_artist = excluded.album_artist, album = excluded.album, \
                track_no = excluded.track_no, disc_no = excluded.disc_no, \
                year = excluded.year, genre = excluded.genre, \
                duration_ms = excluded.duration_ms, sample_rate = excluded.sample_rate, \
                channels = excluded.channels, codec = excluded.codec, \
                rg_track_gain = excluded.rg_track_gain, rg_track_peak = excluded.rg_track_peak, \
                rg_album_gain = excluded.rg_album_gain, rg_album_peak = excluded.rg_album_peak, \
                mtime = excluded.mtime, size = excluded.size",
        )?;
        stmt.execute(params![
            path_to_string(&t.path),
            t.title,
            t.artist,
            t.album_artist,
            t.album,
            t.track_no,
            t.disc_no,
            t.year,
            t.genre,
            t.duration_ms as i64,
            t.sample_rate,
            t.channels,
            t.codec,
            t.replay_gain.track_gain,
            t.replay_gain.track_peak,
            t.replay_gain.album_gain,
            t.replay_gain.album_peak,
            t.mtime,
            t.size as i64,
        ])?;
        Ok(())
    }

    /// Stored `(mtime, size)` of `path`, or `None` if the path is not in the library.
    pub fn stat(&self, path: &Path) -> Result<Option<(i64, u64)>, LibraryError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT mtime, size FROM tracks WHERE path = ?1")?;
        let row = stmt
            .query_row([path_to_string(path)], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as u64))
            })
            .optional()?;
        Ok(row)
    }

    /// Deletes every row whose path is not in `seen`. Returns the number of deleted rows.
    /// Does not open a transaction itself; wrap in [`Library::transaction`] for atomicity.
    pub fn remove_missing(&self, seen: &HashSet<String>) -> Result<usize, LibraryError> {
        let stale: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT id, path FROM tracks")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            let mut stale = Vec::new();
            for row in rows {
                let (id, path) = row?;
                if !seen.contains(&path) {
                    stale.push(id);
                }
            }
            stale
        };
        let mut del = self
            .conn
            .prepare_cached("DELETE FROM tracks WHERE id = ?1")?;
        for id in &stale {
            del.execute([id])?;
        }
        Ok(stale.len())
    }

    /// Runs `f` inside one transaction (much faster than one commit per upsert). Not reentrant.
    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&Library) -> Result<T, LibraryError>,
    ) -> Result<T, LibraryError> {
        self.conn.execute_batch("BEGIN")?;
        match f(self) {
            Ok(v) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn track_count(&self) -> Result<u64, LibraryError> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    pub fn all_tracks(&self) -> Result<Vec<Track>, LibraryError> {
        self.query_tracks(
            &format!(
                "SELECT {TRACK_COLUMNS} FROM tracks \
                 ORDER BY album_artist, album, disc_no, track_no, path"
            ),
            [],
        )
    }

    /// Albums grouped by (album artist, album). Tracks without an album tag are not listed
    /// here; they are still reachable through `all_tracks` and `search`.
    pub fn albums(&self) -> Result<Vec<Album>, LibraryError> {
        let mut stmt = self.conn.prepare(
            "SELECT album, album_artist, MIN(year), COUNT(*), MIN(path) FROM tracks \
             WHERE album IS NOT NULL \
             GROUP BY album_artist, album \
             ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Album {
                name: r.get(0)?,
                artist: r.get(1)?,
                year: r.get(2)?,
                track_count: r.get(3)?,
                first_track_path: PathBuf::from(r.get::<_, String>(4)?),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Tracks of one album ordered by disc then track number. `album_artist` `None` matches
    /// tracks with no album artist (SQL `IS`, so NULL equals NULL).
    pub fn tracks_of_album(
        &self,
        album_artist: Option<&str>,
        album: &str,
    ) -> Result<Vec<Track>, LibraryError> {
        self.query_tracks(
            &format!(
                "SELECT {TRACK_COLUMNS} FROM tracks \
                 WHERE album_artist IS ?1 AND album = ?2 \
                 ORDER BY disc_no, track_no, path"
            ),
            params![album_artist, album],
        )
    }

    /// Case-insensitive substring match on title, artist and album. `%`, `_` and `\` in `q`
    /// are matched literally. (SQLite `LIKE` is only case-insensitive for ASCII.)
    pub fn search(&self, q: &str) -> Result<Vec<Track>, LibraryError> {
        let escaped = q
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        self.query_tracks(
            &format!(
                "SELECT {TRACK_COLUMNS} FROM tracks \
                 WHERE title LIKE ?1 ESCAPE '\\' OR artist LIKE ?1 ESCAPE '\\' \
                    OR album LIKE ?1 ESCAPE '\\' \
                 ORDER BY album_artist, album, disc_no, track_no, path"
            ),
            [pattern],
        )
    }

    fn query_tracks(
        &self,
        sql: &str,
        p: impl rusqlite::Params,
    ) -> Result<Vec<Track>, LibraryError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(p, row_to_track)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

pub(super) fn path_to_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn row_to_track(r: &Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get::<_, i64>(0)? as u64,
        path: PathBuf::from(r.get::<_, String>(1)?),
        title: r.get(2)?,
        artist: r.get(3)?,
        album_artist: r.get(4)?,
        album: r.get(5)?,
        track_no: r.get(6)?,
        disc_no: r.get(7)?,
        year: r.get(8)?,
        genre: r.get(9)?,
        duration_ms: r.get::<_, i64>(10)? as u64,
        sample_rate: r.get(11)?,
        channels: r.get(12)?,
        codec: r.get(13)?,
        replay_gain: ReplayGain {
            track_gain: r.get(14)?,
            track_peak: r.get(15)?,
            album_gain: r.get(16)?,
            album_peak: r.get(17)?,
        },
        mtime: r.get(18)?,
        size: r.get::<_, i64>(19)? as u64,
    })
}

fn migrate(conn: &Connection) -> Result<(), LibraryError> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(LibraryError::SchemaTooNew(version));
    }
    if version < 1 {
        conn.execute_batch(&format!(
            "BEGIN; {MIGRATION_V1} PRAGMA user_version = 1; COMMIT;"
        ))?;
    }
    Ok(())
}
