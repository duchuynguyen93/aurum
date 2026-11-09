use rand::Rng;
use rand::seq::SliceRandom;

use crate::Track;

/// Repeat behaviour when the end of the queue is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

/// Playback queue. `order` holds indices into `tracks` in play order;
/// `pos` points into `order`.
#[derive(Debug, Default)]
pub struct Queue {
    tracks: Vec<Track>,
    order: Vec<usize>,
    pos: Option<usize>,
    repeat: Repeat,
    shuffle: bool,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    pub fn is_shuffled(&self) -> bool {
        self.shuffle
    }

    /// Appends a track at the end of the play order. The first pushed track becomes current.
    pub fn push(&mut self, track: Track) {
        self.tracks.push(track);
        self.order.push(self.tracks.len() - 1);
        if self.pos.is_none() {
            self.pos = Some(0);
        }
    }

    pub fn current(&self) -> Option<&Track> {
        self.pos.map(|p| &self.tracks[self.order[p]])
    }

    /// Advances to the next track and returns it. With `Repeat::One` the same track is returned.
    /// At the end, wraps with `Repeat::All`, otherwise returns `None` and keeps the position.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&Track> {
        let pos = self.pos?;
        if self.repeat != Repeat::One {
            if pos + 1 < self.order.len() {
                self.pos = Some(pos + 1);
            } else if self.repeat == Repeat::All {
                self.pos = Some(0);
            } else {
                return None;
            }
        }
        self.current()
    }

    /// Moves to the previous track and returns it. With `Repeat::One` the same track is returned.
    /// At the start, wraps with `Repeat::All`, otherwise returns `None` and keeps the position.
    pub fn prev(&mut self) -> Option<&Track> {
        let pos = self.pos?;
        if self.repeat != Repeat::One {
            if pos > 0 {
                self.pos = Some(pos - 1);
            } else if self.repeat == Repeat::All {
                self.pos = Some(self.order.len() - 1);
            } else {
                return None;
            }
        }
        self.current()
    }

    /// Replaces the whole queue with `tracks` in insertion order and makes `start` current.
    /// An out-of-range `start` becomes the first track. With shuffle on, the rest is shuffled
    /// and the start track is played first.
    pub fn replace(&mut self, tracks: Vec<Track>, start: usize) {
        self.order = (0..tracks.len()).collect();
        self.tracks = tracks;
        if self.tracks.is_empty() {
            self.pos = None;
            return;
        }
        let start = if start < self.tracks.len() { start } else { 0 };
        self.pos = Some(start);
        if self.shuffle {
            self.set_shuffle(true, &mut rand::rng());
        }
    }

    /// The track `next` would return, without changing the position.
    pub fn peek_next(&self) -> Option<&Track> {
        let pos = self.pos?;
        let next = if self.repeat == Repeat::One {
            pos
        } else if pos + 1 < self.order.len() {
            pos + 1
        } else if self.repeat == Repeat::All {
            0
        } else {
            return None;
        };
        Some(&self.tracks[self.order[next]])
    }

    /// Makes the track at play-order `index` current. `None` (and no change) if out of range.
    pub fn jump(&mut self, index: usize) -> Option<&Track> {
        if index >= self.order.len() {
            return None;
        }
        self.pos = Some(index);
        self.current()
    }

    /// Tracks in play order.
    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.order.iter().map(|&i| &self.tracks[i])
    }

    /// Play-order index of the current track.
    pub fn position(&self) -> Option<usize> {
        self.pos
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
    }

    /// Enables or disables shuffle. Enabling builds a new random order with the current
    /// track moved to the front; disabling restores insertion order and keeps the current track.
    pub fn set_shuffle(&mut self, on: bool, rng: &mut impl Rng) {
        self.shuffle = on;
        let current = self.pos.map(|p| self.order[p]);
        if on {
            self.order.shuffle(rng);
            if let Some(cur) = current {
                let at = self
                    .order
                    .iter()
                    .position(|&i| i == cur)
                    .expect("current in order");
                self.order.swap(0, at);
                self.pos = Some(0);
            }
        } else {
            self.order.sort_unstable();
            self.pos = current;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn track(id: u64) -> Track {
        Track {
            id,
            path: format!("/music/{id}.flac").into(),
            title: format!("Track {id}"),
            duration_ms: 1000,
            ..Track::default()
        }
    }

    fn queue_of(n: u64) -> Queue {
        let mut q = Queue::new();
        for id in 0..n {
            q.push(track(id));
        }
        q
    }

    fn id(q: &Queue) -> Option<u64> {
        q.current().map(|t| t.id)
    }

    #[test]
    fn next_and_prev_stop_at_ends_when_repeat_off() {
        let mut q = queue_of(3);
        assert_eq!(id(&q), Some(0));
        assert_eq!(q.next().map(|t| t.id), Some(1));
        assert_eq!(q.next().map(|t| t.id), Some(2));
        assert!(q.next().is_none());
        assert_eq!(id(&q), Some(2));
        q.prev();
        q.prev();
        assert!(q.prev().is_none());
        assert_eq!(id(&q), Some(0));
    }

    #[test]
    fn repeat_all_wraps_both_directions() {
        let mut q = queue_of(3);
        q.set_repeat(Repeat::All);
        assert_eq!(q.prev().map(|t| t.id), Some(2));
        assert_eq!(q.next().map(|t| t.id), Some(0));
    }

    #[test]
    fn repeat_one_keeps_current() {
        let mut q = queue_of(3);
        q.set_repeat(Repeat::One);
        assert_eq!(q.next().map(|t| t.id), Some(0));
        assert_eq!(q.prev().map(|t| t.id), Some(0));
    }

    #[test]
    fn empty_queue_has_no_current() {
        let mut q = Queue::new();
        assert!(q.current().is_none());
        assert!(q.next().is_none());
        assert!(q.prev().is_none());
    }

    #[test]
    fn shuffle_keeps_current_first_and_visits_every_track_once() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut q = queue_of(20);
        q.next();
        q.next();
        q.next();
        let cur = id(&q);
        q.set_shuffle(true, &mut rng);
        assert_eq!(id(&q), cur);
        let mut seen = vec![id(&q).unwrap()];
        while let Some(t) = q.next() {
            seen.push(t.id);
        }
        assert_eq!(seen.len(), 20);
        seen.sort_unstable();
        assert_eq!(seen, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_is_deterministic_for_a_seed_and_reorders() {
        let order_for = |seed| {
            let mut q = queue_of(20);
            q.set_shuffle(true, &mut StdRng::seed_from_u64(seed));
            let mut ids = vec![id(&q).unwrap()];
            while let Some(t) = q.next() {
                ids.push(t.id);
            }
            ids
        };
        assert_eq!(order_for(7), order_for(7));
        assert_ne!(order_for(7), (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn disabling_shuffle_restores_order_and_keeps_current() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut q = queue_of(10);
        q.set_shuffle(true, &mut rng);
        q.next();
        q.next();
        let cur = id(&q);
        q.set_shuffle(false, &mut rng);
        assert_eq!(id(&q), cur);
        let c = cur.unwrap();
        if c + 1 < 10 {
            assert_eq!(q.next().map(|t| t.id), Some(c + 1));
        }
    }

    #[test]
    fn replace_sets_start_and_resets_order() {
        let mut q = queue_of(2);
        q.replace((10..15).map(track).collect(), 3);
        assert_eq!(id(&q), Some(13));
        assert_eq!(q.position(), Some(3));
        assert_eq!(q.len(), 5);
        q.replace(vec![track(1)], 9);
        assert_eq!(id(&q), Some(1));
        q.replace(Vec::new(), 0);
        assert!(q.current().is_none());
        assert_eq!(q.position(), None);
    }

    #[test]
    fn replace_with_shuffle_plays_start_first() {
        let mut q = Queue::new();
        q.set_shuffle(true, &mut StdRng::seed_from_u64(3));
        q.replace((0..20).map(track).collect(), 7);
        assert_eq!(id(&q), Some(7));
        assert_eq!(q.position(), Some(0));
        let mut ids: Vec<u64> = q.tracks().map(|t| t.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn peek_next_matches_next_without_moving() {
        let mut q = queue_of(3);
        for repeat in [Repeat::Off, Repeat::All, Repeat::One] {
            q.set_repeat(repeat);
            q.jump(0);
            for _ in 0..5 {
                let before = q.position();
                let peeked = q.peek_next().map(|t| t.id);
                assert_eq!(q.position(), before);
                assert_eq!(q.next().map(|t| t.id), peeked, "{repeat:?}");
            }
        }
    }

    #[test]
    fn jump_and_tracks_follow_play_order() {
        let mut q = queue_of(4);
        assert_eq!(q.jump(2).map(|t| t.id), Some(2));
        assert!(q.jump(4).is_none());
        assert_eq!(q.position(), Some(2));
        q.set_shuffle(true, &mut StdRng::seed_from_u64(5));
        let order: Vec<u64> = q.tracks().map(|t| t.id).collect();
        assert_eq!(order[q.position().unwrap()], id(&q).unwrap());
    }
}
