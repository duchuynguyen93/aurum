//! The single application model. It is also the root view (`ui::shell` implements `Render`),
//! so every handler can use `cx.listener` and reach the whole state.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use aurum_audio::{AudioEngine, CpalEngine, Event, NullEngine, Receiver, ReplayGain};
use aurum_core::library::{Album, Library, LibraryError, ScanEvent, ScanSummary, Scanner};
use aurum_core::{Queue, Repeat, Track};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext, Bounds, Context, Entity, FocusHandle, PathPromptOptions, Pixels, ScrollHandle,
    UniformListScrollHandle, Window,
};
use rand::Rng;

use super::cover_cache::{self, CoverCache, CoverKey};
use super::paths;
use super::settings::Settings;

const TICK: Duration = Duration::from_millis(50);
/// `prev` restarts the current track instead of going back when played past this point.
const PREV_RESTART_AFTER: Duration = Duration::from_secs(3);
/// Minimum gap between engine seeks while the progress handle is dragged.
const SCRUB_SEEK_INTERVAL: Duration = Duration::from_millis(120);
/// A drag that never reported its mouse-up is abandoned after this long without a drag.
const SCRUB_STALE_AFTER: Duration = Duration::from_secs(2);
const SETTINGS_SAVE_DELAY: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Albums,
    Artists,
    Tracks,
    Settings,
}

/// A page pushed on top of the current list view; `back` pops it. Opening an album from an
/// artist page keeps the artist page underneath.
#[derive(Clone)]
pub enum Detail {
    Album(Rc<AlbumDetail>),
    Artist(String),
}

/// An album page: its tracks (already ordered by disc and track number) and totals.
pub struct AlbumDetail {
    pub artist: Option<String>,
    pub name: String,
    pub year: Option<i32>,
    pub tracks: Vec<Track>,
    pub total: Duration,
    pub scroll: ScrollHandle,
}

impl AlbumDetail {
    pub fn cover_key(&self) -> CoverKey {
        (self.artist.clone().unwrap_or_default(), self.name.clone())
    }

    pub fn cover_path(&self) -> Option<PathBuf> {
        self.tracks.first().map(|t| t.path.clone())
    }

    /// Number of distinct discs; a missing disc number counts as disc 1.
    pub fn disc_count(&self) -> usize {
        self.tracks
            .iter()
            .map(|t| t.disc_no.unwrap_or(1))
            .collect::<HashSet<_>>()
            .len()
    }
}

/// One row of the Artists list (artists are album artists).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistRow {
    pub name: String,
    pub albums: usize,
    pub tracks: usize,
}

/// Groups tracks by album artist, sorted by name ignoring case. Tracks without an album artist
/// are left out (the scanner fills it from the artist tag when it is missing).
pub fn artists_from(tracks: &[Track]) -> Vec<ArtistRow> {
    let mut by_name: HashMap<&str, (HashSet<&str>, usize)> = HashMap::new();
    for t in tracks {
        let Some(name) = t.album_artist.as_deref() else {
            continue;
        };
        let entry = by_name.entry(name).or_default();
        if let Some(album) = t.album.as_deref() {
            entry.0.insert(album);
        }
        entry.1 += 1;
    }
    let mut rows: Vec<ArtistRow> = by_name
        .into_iter()
        .map(|(name, (albums, tracks))| ArtistRow {
            name: name.to_string(),
            albums: albums.len(),
            tracks,
        })
        .collect();
    rows.sort_by_cached_key(|r| (r.name.to_lowercase(), r.name.clone()));
    rows
}

#[derive(Debug, Clone, Default)]
pub struct Playback {
    pub playing: bool,
    pub position: Duration,
    pub duration: Duration,
    pub current: Option<Track>,
    /// The queue ran out; the next play restarts the current track.
    pub ended: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ScanProgress {
    pub scanned: usize,
    pub total_hint: usize,
    pub added: usize,
    pub updated: usize,
}

pub struct AppState {
    pub settings: Settings,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub artists: Vec<ArtistRow>,
    pub library_loaded: bool,
    pub queue: Queue,
    pub playback: Playback,
    pub scan: Option<ScanProgress>,
    pub last_scan: Option<ScanSummary>,
    pub view: View,
    pub detail: Vec<Detail>,
    pub covers: CoverCache,
    /// Lower-cased trimmed query; empty means no filter.
    pub search: String,
    /// Tracks matching `search` (`Library::search`); `None` until the first answer arrives.
    pub search_hits: Option<Vec<Track>>,
    /// Indices into `albums` / `artists` that pass the current filter.
    pub shown_albums: Vec<usize>,
    pub shown_artists: Vec<usize>,
    pub search_input: Entity<InputState>,
    /// Set when the audio device could not be opened or the engine reported an error.
    pub engine_error: Option<String>,
    /// Library database problems (open or query failures).
    pub library_error: Option<String>,

    pub focus: FocusHandle,
    pub tracks_scroll: UniformListScrollHandle,
    pub albums_scroll: UniformListScrollHandle,
    pub artists_scroll: UniformListScrollHandle,
    pub artist_albums_scroll: UniformListScrollHandle,
    /// Set on mouse-down in a drag strip; the first mouse-move then starts the window move.
    pub drag_armed: Rc<Cell<bool>>,
    /// Ratio shown on the progress bar while it is being dragged.
    pub scrub: Option<f32>,
    pub progress_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub volume_bounds: Rc<Cell<Bounds<Pixels>>>,

    engine: Box<dyn AudioEngine>,
    engine_rx: Receiver<Event>,
    scan_rx: Option<crossbeam_channel::Receiver<ScanEvent>>,
    settings_path: PathBuf,
    db_path: PathBuf,
    settings_dirty_since: Option<Instant>,
    search_gen: u64,
    rescan_pending: bool,
    scrub_started: Instant,
    last_scrub_seek: Instant,
}

fn gain_of(track: &Track) -> Option<ReplayGain> {
    track.replay_gain.track_gain.map(|gain_db| ReplayGain {
        gain_db,
        peak: track.replay_gain.track_peak,
    })
}

impl AppState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings_path = paths::settings_path();
        let db_path = paths::db_path();
        if let Err(e) = std::fs::create_dir_all(paths::data_dir()) {
            log::warn!("cannot create data dir: {e}");
        }
        let settings = Settings::load(&settings_path);

        let mut engine_error = None;
        let mut engine: Box<dyn AudioEngine> = match CpalEngine::new() {
            Ok(e) => Box::new(e),
            Err(e) => {
                log::error!("audio output unavailable: {e}");
                engine_error = Some(format!("Audio output unavailable: {e}"));
                Box::new(NullEngine)
            }
        };
        engine.set_volume(settings.volume);
        let engine_rx = engine.events();

        let mut queue = Queue::new();
        queue.set_repeat(settings.repeat.into());
        queue.set_shuffle(settings.shuffle, &mut rand::rng());

        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        cx.subscribe_in(
            &search_input,
            window,
            |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    this.set_search(&text, cx);
                }
            },
        )
        .detach();

        let now = Instant::now();
        let mut this = Self {
            settings,
            albums: Vec::new(),
            tracks: Vec::new(),
            artists: Vec::new(),
            library_loaded: false,
            queue,
            playback: Playback::default(),
            scan: None,
            last_scan: None,
            view: View::Albums,
            detail: Vec::new(),
            covers: CoverCache::default(),
            search: String::new(),
            search_hits: None,
            shown_albums: Vec::new(),
            shown_artists: Vec::new(),
            search_input,
            engine_error,
            library_error: None,
            focus: cx.focus_handle(),
            tracks_scroll: UniformListScrollHandle::new(),
            albums_scroll: UniformListScrollHandle::new(),
            artists_scroll: UniformListScrollHandle::new(),
            artist_albums_scroll: UniformListScrollHandle::new(),
            drag_armed: Rc::default(),
            scrub: None,
            progress_bounds: Rc::default(),
            volume_bounds: Rc::default(),
            engine,
            engine_rx,
            scan_rx: None,
            settings_path,
            db_path,
            settings_dirty_since: None,
            search_gen: 0,
            rescan_pending: false,
            scrub_started: now,
            last_scrub_seek: now,
        };

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |s, cx| s.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();

        this.reload_library(cx);
        this.start_scan(cx);
        this
    }

    // ---- library -------------------------------------------------------------------------

    /// Re-reads albums and tracks on a background thread.
    pub fn reload_library(&mut self, cx: &mut Context<Self>) {
        let db = self.db_path.clone();
        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let lib = Library::open(&db)?;
                    Ok::<_, LibraryError>((lib.albums()?, lib.all_tracks()?))
                })
                .await;
            this.update(cx, |s, cx| {
                match loaded {
                    Ok((albums, tracks)) => {
                        log::info!(
                            "library loaded: {} tracks, {} albums",
                            tracks.len(),
                            albums.len()
                        );
                        s.artists = artists_from(&tracks);
                        s.albums = albums;
                        s.tracks = tracks;
                        s.library_error = None;
                        s.refresh_filters();
                    }
                    Err(e) => {
                        log::error!("cannot read library: {e}");
                        s.library_error = Some(format!("Library database error: {e}"));
                    }
                }
                s.library_loaded = true;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn start_scan(&mut self, cx: &mut Context<Self>) {
        if !self.settings.roots.is_empty() {
            self.run_scan(cx);
        }
    }

    /// Scans the current roots, also when there are none (which purges the library).
    /// A request during a running scan is repeated once that scan finishes.
    fn run_scan(&mut self, cx: &mut Context<Self>) {
        if self.scan.is_some() {
            self.rescan_pending = true;
            return;
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        // The scanner owns its thread and ends with `Finished`; the handle is not needed.
        drop(Scanner::new(&self.db_path).scan(&self.settings.roots, tx));
        self.scan_rx = Some(rx);
        self.scan = Some(ScanProgress::default());
        log::info!("scan started: {} root(s)", self.settings.roots.len());
        cx.notify();
    }

    pub fn add_roots(&mut self, new: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut added = false;
        for p in new {
            if !self.settings.roots.contains(&p) {
                self.settings.roots.push(p);
                added = true;
            }
        }
        if added {
            self.save_settings(cx);
            self.start_scan(cx);
        }
        cx.notify();
    }

    pub fn remove_root(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.settings.roots.len() {
            self.settings.roots.remove(index);
            self.save_settings(cx);
            // The scanner deletes rows outside the roots, so this also drops the removed
            // folder's tracks (everything, when no root is left).
            self.run_scan(cx);
            cx.notify();
        }
    }

    /// Opens the system folder picker (multiple selection allowed).
    pub fn prompt_add_folder(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Add".into()),
        });
        cx.spawn(async move |this, cx| match rx.await {
            Ok(Ok(Some(paths))) => {
                this.update(cx, |s, cx| s.add_roots(paths, cx)).ok();
            }
            Ok(Ok(None)) => {}
            Ok(Err(e)) => log::warn!("folder picker failed: {e}"),
            Err(_) => log::warn!("folder picker closed without a result"),
        })
        .detach();
    }

    /// Switches the top-level view and drops any detail pages on top of it.
    pub fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        if self.view != view || !self.detail.is_empty() {
            self.view = view;
            self.detail.clear();
            cx.notify();
        }
    }

    // ---- navigation ----------------------------------------------------------------------

    /// Opens the page of `self.albums[index]`. Its tracks come from the in-memory list, which
    /// is already ordered by album, disc and track number.
    pub fn open_album(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(album) = self.albums.get(index) else {
            return;
        };
        let tracks: Vec<Track> = self
            .tracks
            .iter()
            .filter(|t| {
                t.album.as_deref() == Some(album.name.as_str()) && t.album_artist == album.artist
            })
            .cloned()
            .collect();
        let total = Duration::from_millis(tracks.iter().map(|t| t.duration_ms).sum());
        let detail = AlbumDetail {
            artist: album.artist.clone(),
            name: album.name.clone(),
            year: album.year,
            tracks,
            total,
            scroll: ScrollHandle::new(),
        };
        if let Some(path) = detail.cover_path() {
            self.ensure_cover_for(detail.cover_key(), path, cx);
        }
        self.detail.push(Detail::Album(Rc::new(detail)));
        cx.notify();
    }

    pub fn open_artist(&mut self, name: String, cx: &mut Context<Self>) {
        self.artist_albums_scroll = UniformListScrollHandle::new();
        self.detail.push(Detail::Artist(name));
        cx.notify();
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        if self.detail.pop().is_some() {
            cx.notify();
        }
    }

    /// Albums shown in the grid of the current page: every filtered album, or the filtered
    /// albums of the artist whose page is open.
    pub fn grid_albums(&self) -> Vec<usize> {
        match self.detail.last() {
            Some(Detail::Artist(name)) => self
                .shown_albums
                .iter()
                .copied()
                .filter(|&i| self.albums[i].artist.as_deref() == Some(name.as_str()))
                .collect(),
            _ => self.shown_albums.clone(),
        }
    }

    // ---- search --------------------------------------------------------------------------

    /// Tracks of the Tracks view: all of them, or the search hits.
    pub fn visible_tracks(&self) -> &[Track] {
        match (&self.search_hits, self.search.is_empty()) {
            (Some(hits), false) => hits,
            _ => &self.tracks,
        }
    }

    /// Applies a new query typed in the search box. Track matches come from `Library::search`
    /// on a background thread; answers to an older query are dropped.
    pub fn set_search(&mut self, text: &str, cx: &mut Context<Self>) {
        let query = text.trim().to_lowercase();
        if query == self.search {
            return;
        }
        self.search = query.clone();
        self.search_gen += 1;
        if query.is_empty() {
            self.search_hits = None;
            self.refresh_filters();
            cx.notify();
            return;
        }
        // Names can be filtered at once; track matches follow when the query answers.
        self.refresh_filters();
        cx.notify();
        let (gen_id, db) = (self.search_gen, self.db_path.clone());
        cx.spawn(async move |this, cx| {
            let hits = cx
                .background_executor()
                .spawn(async move { Library::open(&db)?.search(&query) })
                .await;
            this.update(cx, |s, cx| {
                if s.search_gen != gen_id {
                    return;
                }
                s.search_hits = Some(hits.unwrap_or_else(|e| {
                    log::error!("search failed: {e}");
                    Vec::new()
                }));
                s.refresh_filters();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Esc: empties the box, drops the filter and hands the focus back to the shell.
    pub fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.set_search("", cx);
        window.focus(&self.focus, cx);
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input
            .update(cx, |input, cx| input.focus(window, cx));
    }

    /// Recomputes `shown_albums` and `shown_artists` from the query: an album passes when its
    /// name or artist contains it, or when one of its tracks is a search hit.
    fn refresh_filters(&mut self) {
        if self.search.is_empty() {
            self.shown_albums = (0..self.albums.len()).collect();
            self.shown_artists = (0..self.artists.len()).collect();
            return;
        }
        let q = &self.search;
        let hit_albums: HashSet<(Option<&str>, &str)> = self
            .search_hits
            .iter()
            .flatten()
            .filter_map(|t| Some((t.album_artist.as_deref(), t.album.as_deref()?)))
            .collect();
        let shown_albums = self
            .albums
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a.name.to_lowercase().contains(q.as_str())
                    || a.artist
                        .as_deref()
                        .is_some_and(|x| x.to_lowercase().contains(q.as_str()))
                    || hit_albums.contains(&(a.artist.as_deref(), a.name.as_str()))
            })
            .map(|(i, _)| i)
            .collect();
        let shown_artists = self
            .artists
            .iter()
            .enumerate()
            .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
            .map(|(i, _)| i)
            .collect();
        self.shown_albums = shown_albums;
        self.shown_artists = shown_artists;
    }

    // ---- settings ------------------------------------------------------------------------

    /// Writes settings on a background thread.
    fn save_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_dirty_since = None;
        let settings = self.settings.clone();
        let path = self.settings_path.clone();
        cx.background_executor()
            .spawn(async move {
                if let Err(e) = settings.save(&path) {
                    log::warn!("cannot save settings: {e}");
                }
            })
            .detach();
    }

    // ---- playback ------------------------------------------------------------------------

    /// Replaces the queue with `tracks` and plays `start`.
    pub fn play_tracks(&mut self, tracks: Vec<Track>, start: usize, cx: &mut Context<Self>) {
        self.queue.replace(tracks, start);
        self.start_current(cx);
    }

    /// Plays row `index` of the Tracks list (as filtered by the search).
    pub fn play_library_track(&mut self, index: usize, cx: &mut Context<Self>) {
        let tracks = self.visible_tracks().to_vec();
        self.play_tracks(tracks, index, cx);
    }

    /// Turns shuffle on and plays `tracks` from a random one.
    pub fn play_shuffled(&mut self, tracks: Vec<Track>, cx: &mut Context<Self>) {
        if tracks.is_empty() {
            return;
        }
        if !self.queue.is_shuffled() {
            self.queue.set_shuffle(true, &mut rand::rng());
            self.settings.shuffle = true;
            self.save_settings(cx);
        }
        let start = rand::rng().random_range(0..tracks.len());
        self.play_tracks(tracks, start, cx);
    }

    /// Loads and plays the queue's current track.
    fn start_current(&mut self, cx: &mut Context<Self>) {
        let Some(track) = self.queue.current().cloned() else {
            return;
        };
        log::info!("play: {}", track.path.display());
        self.engine.load(&track.path, gain_of(&track));
        self.engine.play();
        self.playback.playing = true;
        self.playback.ended = false;
        self.playback.position = Duration::ZERO;
        self.playback.duration = Duration::from_millis(track.duration_ms);
        self.scrub = None;
        self.ensure_cover(&track, cx);
        self.playback.current = Some(track);
        self.queue_next_after();
        cx.notify();
    }

    /// Preloads whatever plays after the current track. Called after every change of the
    /// current track, shuffle or repeat.
    fn queue_next_after(&mut self) {
        if let Some(next) = self.queue.peek_next() {
            self.engine
                .queue_next_with_gain(&next.path.clone(), gain_of(next));
        }
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if self.playback.current.is_none() {
            self.start_current(cx);
            return;
        }
        if self.playback.ended {
            self.start_current(cx);
        } else if self.playback.playing {
            self.engine.pause();
            self.playback.playing = false;
        } else {
            self.engine.play();
            self.playback.playing = true;
        }
        cx.notify();
    }

    pub fn next(&mut self, cx: &mut Context<Self>) {
        if self.queue.next().is_some() {
            self.start_current(cx);
        }
    }

    pub fn prev(&mut self, cx: &mut Context<Self>) {
        if self.playback.position > PREV_RESTART_AFTER || self.queue.prev().is_none() {
            self.seek_to(Duration::ZERO, cx);
        } else {
            self.start_current(cx);
        }
    }

    fn seek_to(&mut self, position: Duration, cx: &mut Context<Self>) {
        if self.playback.current.is_none() || self.playback.ended {
            return;
        }
        self.engine.seek(position);
        self.playback.position = position;
        cx.notify();
    }

    fn position_at(&self, ratio: f32) -> Duration {
        self.playback.duration.mul_f32(ratio.clamp(0.0, 1.0))
    }

    /// Progress handle moved or pressed: update the display and seek (rate limited).
    pub fn scrub_to(&mut self, ratio: f32, cx: &mut Context<Self>) {
        if self.playback.current.is_none() {
            return;
        }
        let ratio = ratio.clamp(0.0, 1.0);
        if self.scrub.is_none() {
            self.scrub_started = Instant::now();
        }
        self.scrub = Some(ratio);
        self.playback.position = self.position_at(ratio);
        if self.last_scrub_seek.elapsed() >= SCRUB_SEEK_INTERVAL && !self.playback.ended {
            self.last_scrub_seek = Instant::now();
            self.engine.seek(self.playback.position);
        }
        cx.notify();
    }

    /// Mouse released: the final seek, then the engine position drives the bar again.
    pub fn scrub_end(&mut self, cx: &mut Context<Self>) {
        if let Some(ratio) = self.scrub.take() {
            let position = self.position_at(ratio);
            self.seek_to(position, cx);
            self.last_scrub_seek = Instant::now();
        }
    }

    pub fn set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        let volume = volume.clamp(0.0, 1.0);
        self.settings.volume = volume;
        self.engine.set_volume(volume);
        self.settings_dirty_since = Some(Instant::now());
        cx.notify();
    }

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        let v = if self.settings.volume > 0.0 { 0.0 } else { 0.8 };
        self.set_volume(v, cx);
    }

    pub fn toggle_shuffle(&mut self, cx: &mut Context<Self>) {
        let on = !self.queue.is_shuffled();
        self.queue.set_shuffle(on, &mut rand::rng());
        self.settings.shuffle = on;
        self.queue_next_after();
        self.save_settings(cx);
        cx.notify();
    }

    pub fn cycle_repeat(&mut self, cx: &mut Context<Self>) {
        let next = match self.queue.repeat() {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        };
        self.queue.set_repeat(next);
        self.settings.repeat = next.into();
        self.queue_next_after();
        self.save_settings(cx);
        cx.notify();
    }

    // ---- covers --------------------------------------------------------------------------

    fn ensure_cover(&mut self, track: &Track, cx: &mut Context<Self>) {
        self.ensure_cover_for(cover_cache::key_of(track), track.path.clone(), cx);
    }

    /// Starts loading the cover under `key` from the picture embedded in `path`, once.
    pub fn ensure_cover_for(&mut self, key: CoverKey, path: PathBuf, cx: &mut Context<Self>) {
        if !self.covers.begin_load(&key) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let art = cx
                .background_executor()
                .spawn(async move { cover_cache::load_art(&path) })
                .await;
            this.update(cx, |s, cx| {
                s.covers.finish_load(key, art);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---- bridge --------------------------------------------------------------------------

    /// Runs every 50 ms: drains engine and scan events into the model.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;

        while let Ok(event) = self.engine_rx.try_recv() {
            changed = true;
            self.on_engine_event(event, cx);
        }

        let mut scan_events = Vec::new();
        if let Some(rx) = &self.scan_rx {
            while let Ok(e) = rx.try_recv() {
                scan_events.push(e);
            }
        }
        for event in scan_events {
            changed = true;
            self.on_scan_event(event, cx);
        }

        if self.scrub.is_some()
            && !cx.has_active_drag()
            && self.scrub_started.elapsed() > SCRUB_STALE_AFTER
        {
            self.scrub_end(cx);
            changed = true;
        }
        if self
            .settings_dirty_since
            .is_some_and(|t| t.elapsed() >= SETTINGS_SAVE_DELAY)
        {
            self.save_settings(cx);
        }
        if changed {
            cx.notify();
        }
    }

    fn on_engine_event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Position(p) => {
                if self.scrub.is_none() {
                    self.playback.position = p;
                }
            }
            Event::Loaded { path, duration, .. } => {
                log::info!("loaded: {} ({:?})", path.display(), duration);
                self.playback.duration = duration;
                self.engine_error = None;
            }
            Event::TrackEnded { path } => {
                log::info!("track ended: {}", path.display());
                if self.queue.next().is_none() {
                    self.playback.playing = false;
                    self.playback.ended = true;
                    self.playback.position = Duration::ZERO;
                }
            }
            Event::TrackStarted { path } => {
                log::info!("track started: {}", path.display());
                let track = self
                    .queue
                    .current()
                    .filter(|t| t.path == path)
                    .cloned()
                    .or_else(|| self.tracks.iter().find(|t| t.path == path).cloned());
                if let Some(track) = track {
                    self.playback.duration = Duration::from_millis(track.duration_ms);
                    self.playback.position = Duration::ZERO;
                    self.playback.playing = true;
                    self.playback.ended = false;
                    self.ensure_cover(&track, cx);
                    self.playback.current = Some(track);
                }
                self.queue_next_after();
            }
            Event::Error(msg) => {
                log::warn!("engine error: {msg}");
                self.engine_error = Some(msg);
            }
            Event::Underrun => log::debug!("underrun"),
        }
    }

    fn on_scan_event(&mut self, event: ScanEvent, cx: &mut Context<Self>) {
        match event {
            ScanEvent::Started { total_hint } => {
                if let Some(s) = &mut self.scan {
                    s.total_hint = total_hint;
                }
            }
            ScanEvent::Progress {
                scanned,
                added,
                updated,
            } => {
                if let Some(s) = &mut self.scan {
                    s.scanned = scanned;
                    s.added = added;
                    s.updated = updated;
                }
            }
            ScanEvent::Finished { summary } => {
                log::info!(
                    "scan finished: scanned {} added {} updated {} unchanged {} removed {} errors {}",
                    summary.scanned,
                    summary.added,
                    summary.updated,
                    summary.unchanged,
                    summary.removed,
                    summary.errors
                );
                self.scan = None;
                self.scan_rx = None;
                self.last_scan = Some(summary);
                self.reload_library(cx);
                if std::mem::take(&mut self.rescan_pending) {
                    self.run_scan(cx);
                }
            }
            ScanEvent::Error { path, msg } => {
                log::warn!("scan error at {}: {msg}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(album_artist: Option<&str>, album: Option<&str>) -> Track {
        Track {
            album_artist: album_artist.map(String::from),
            album: album.map(String::from),
            ..Track::default()
        }
    }

    #[test]
    fn artists_group_by_album_artist_and_sort_ignoring_case() {
        let tracks = vec![
            track(Some("beta"), Some("B1")),
            track(Some("Alpha"), Some("A1")),
            track(Some("Alpha"), Some("A1")),
            track(Some("Alpha"), Some("A2")),
            track(Some("Alpha"), None),
            track(None, Some("Orphan")),
        ];
        let rows = artists_from(&tracks);
        assert_eq!(
            rows,
            vec![
                ArtistRow {
                    name: "Alpha".into(),
                    albums: 2,
                    tracks: 4
                },
                ArtistRow {
                    name: "beta".into(),
                    albums: 1,
                    tracks: 1
                },
            ]
        );
    }
}
