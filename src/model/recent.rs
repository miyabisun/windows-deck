//! Which of the things used lately to keep: the most recent few, and always those of the frame
//! being drawn and the one before it (a list's pictures), so that nothing on the screen is
//! forgotten and read again.

use std::{collections::HashMap, hash::Hash};

pub struct Recent<K> {
    capacity: usize,
    /// The frame being drawn; it moves on only after something was used in it.
    frame: u64,
    used_in_frame: bool,
    clock: u64,
    /// When each was last used: its frame and the clock's tick.
    used: HashMap<K, (u64, u64)>,
}

impl<K: Copy + Eq + Hash> Recent<K> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            frame: 0,
            used_in_frame: false,
            clock: 0,
            used: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.used.len()
    }

    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }

    /// A new frame starts.
    pub fn next_frame(&mut self) {
        if self.used_in_frame {
            self.frame += 1;
            self.used_in_frame = false;
        }
    }

    /// `key` is used now. Returns what to forget: the least recently used beyond the capacity.
    pub fn touch(&mut self, key: K) -> Vec<K> {
        self.used_in_frame = true;
        self.clock += 1;
        self.used.insert(key, (self.frame, self.clock));
        let mut forgotten = Vec::new();
        while self.used.len() > self.capacity {
            let oldest = self.used.iter().min_by_key(|(_, (_, tick))| *tick);
            let Some((&key, &(frame, _))) = oldest else {
                break;
            };
            if frame + 1 >= self.frame {
                break;
            }
            self.used.remove(&key);
            forgotten.push(key);
        }
        forgotten
    }
}

#[cfg(test)]
mod tests {
    use super::Recent;

    /// Use each of `keys` in a frame of its own; what was forgotten, in order.
    fn one_a_frame(recent: &mut Recent<u32>, keys: &[u32]) -> Vec<u32> {
        let mut forgotten = Vec::new();
        for &key in keys {
            recent.next_frame();
            forgotten.extend(recent.touch(key));
        }
        forgotten
    }

    #[test]
    fn keeps_the_most_recently_used() {
        let mut recent = Recent::new(2);
        assert_eq!(one_a_frame(&mut recent, &[1, 2, 3, 4, 5]), [1, 2, 3]);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn using_one_again_keeps_it() {
        let mut recent = Recent::new(3);
        assert_eq!(
            one_a_frame(&mut recent, &[1, 2, 3, 1, 4, 5]),
            [2, 3],
            "1 was used after 2 and 3, so it outlives them"
        );
    }

    #[test]
    fn never_forgets_what_this_frame_or_the_one_before_used() {
        let mut recent = Recent::new(2);
        // A screen of four pictures against a capacity of two: every frame uses them all.
        for _ in 0..3 {
            recent.next_frame();
            for key in [1, 2, 3, 4] {
                assert_eq!(recent.touch(key), [] as [u32; 0], "{key} is on the screen");
            }
        }
        assert_eq!(recent.len(), 4);
        // The screen moves on to other pictures: the old ones go once two frames have passed.
        recent.next_frame();
        assert_eq!(
            recent.touch(5),
            [] as [u32; 0],
            "1 to 4 were on the last frame"
        );
        recent.next_frame();
        assert_eq!(
            recent.touch(5),
            [1, 2, 3],
            "down to the capacity, oldest first"
        );
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn a_frame_that_used_nothing_does_not_count() {
        let mut recent = Recent::new(1);
        recent.next_frame();
        assert!(recent.touch(1).is_empty(), "the first picture is kept");
        // Frames may start without drawing the list; 1 is still the last frame's picture.
        for _ in 0..5 {
            recent.next_frame();
        }
        assert_eq!(recent.touch(2), [] as [u32; 0]);
        recent.next_frame();
        assert_eq!(recent.touch(2), [1]);
    }
}
