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
            artist: None,
            album: None,
            duration_ms: 1000,
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
}
