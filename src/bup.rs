use super::Engine;
use std::default::Default;
use std::mem;

pub type Digest = u32;

const WINDOW_BITS: usize = 6;
const WINDOW_SIZE: usize = 1 << WINDOW_BITS;

const CHAR_OFFSET: usize = 31;

/// Default chunk size used by `bup`
pub const CHUNK_SIZE: u32 = 1 << CHUNK_BITS;

/// Default chunk size used by `bup` (log2)
pub const CHUNK_BITS: u32 = 13;

/// Rolling checksum method used by `bup`
///
/// Strongly based on
/// https://github.com/bup/bup/blob/706e8d273/lib/bup/bupsplit.c
/// https://github.com/bup/bup/blob/706e8d273/lib/bup/bupsplit.h
/// (a bit like https://godoc.org/camlistore.org/pkg/rollsum)
pub struct Bup {
    state: State,
    window: [u8; WINDOW_SIZE],
    wofs: usize,
    chunk_bits: u32,
}

struct State {
    s1: u32,
    s2: u32,
}

impl State {
    const fn new() -> Self {
        Self {
            s1: (WINDOW_SIZE * CHAR_OFFSET) as u32,
            s2: (WINDOW_SIZE * (WINDOW_SIZE - 1) * CHAR_OFFSET) as u32,
        }
    }

    #[inline(always)]
    fn add(&mut self, drop: u8, add: u8) {
        self.s1 += add as u32;
        self.s1 -= drop as u32;
        self.s2 += self.s1;
        self.s2 -= (WINDOW_SIZE * (drop as usize + CHAR_OFFSET)) as u32;
    }

    fn digest(&self) -> Digest {
        ((self.s1 as Digest) << 16) | ((self.s2 as Digest) & 0xffff)
    }
}

impl Default for Bup {
    fn default() -> Self {
        Bup {
            state: State::new(),
            window: [0; WINDOW_SIZE],
            wofs: 0,
            chunk_bits: CHUNK_BITS,
        }
    }
}

impl Engine for Bup {
    type Digest = Digest;

    #[inline(always)]
    fn roll_byte(&mut self, newch: u8) {
        // Since this crate is performance ciritical, and
        // we're in strict control of `wofs`, it is justified
        // to skip bound checking to increase the performance
        debug_assert!(self.wofs < self.window.len());
        let slot: &mut u8 = unsafe { self.window.get_unchecked_mut(self.wofs) };
        let prevch = mem::replace(slot, newch);
        self.state.add(prevch, newch);
        self.wofs = (self.wofs + 1) % WINDOW_SIZE;
    }

    fn roll(&mut self, buf: &[u8]) {
        crate::roll_windowed(self, WINDOW_SIZE, buf);
    }

    #[inline(always)]
    fn digest(&self) -> Digest {
        self.state.digest()
    }

    #[inline]
    fn reset(&mut self) {
        *self = Bup {
            chunk_bits: self.chunk_bits,
            ..Default::default()
        }
    }

    fn find_chunk_edge_cond<F>(&mut self, buf: &[u8], cond: F) -> Option<(usize, Self::Digest)>
    where
        F: Fn(&Self) -> bool,
    {
        let mut incoming_bytes = buf.iter().copied().enumerate();
        let outgoing_slices = &[&self.window[self.wofs..], &self.window[..self.wofs], buf];

        for &outgoing_slice in outgoing_slices {
            for &outgoing in outgoing_slice {
                let (i, incoming) = match incoming_bytes.next() {
                    Some(v) => v,
                    None => {
                        self.add_to_window(buf);
                        return None;
                    }
                };
                self.state.add(outgoing, incoming);
                if cond(self) {
                    let digest = self.digest();
                    let end = i + 1;
                    self.reset();
                    return Some((end, digest));
                }
            }
        }
        // the last outgoing slice is as long as incoming
        unreachable!();
    }
}

impl Bup {
    /// Create new Bup engine with default chunking settings
    pub fn new() -> Self {
        Default::default()
    }

    /// Create new Bup engine with custom chunking settings
    ///
    /// `chunk_bits` is number of bits that need to match in
    /// the edge condition. `CHUNK_BITS` constant is the default.
    pub fn new_with_chunk_bits(chunk_bits: u32) -> Self {
        assert!(chunk_bits < 32);
        Bup {
            chunk_bits,
            ..Default::default()
        }
    }

    /// Find chunk edge using Bup defaults.
    ///
    /// See `Engine::find_chunk_edge_cond`.
    pub fn find_chunk_edge(&mut self, buf: &[u8]) -> Option<(usize, Digest)> {
        let chunk_mask = (1 << self.chunk_bits) - 1;
        self.find_chunk_edge_cond(buf, |e: &Bup| e.digest() & chunk_mask == chunk_mask)
    }

    /// Counts the number of low bits set in the rollsum, assuming
    /// the digest has the bottom `CHUNK_BITS` bits set to `1`
    /// (i.e. assuming a digest at a default bup chunk edge, as
    /// returned by `find_chunk_edge`).
    /// Be aware that there's a deliberate 'bug' in this function
    /// in order to match expected return values from other bupsplit
    /// implementations.
    // Note: because of the state is reset after finding an edge, assist
    // users use this correctly by making them pass in a digest they've
    // obtained.
    pub fn count_bits(&self, digest: Digest) -> u32 {
        let rsum = digest >> self.chunk_bits;

        // Ignore the next bit as well. This isn't actually
        // a problem as the distribution of values will be the same,
        // but it is unexpected.
        let rsum = rsum >> 1;
        rsum.trailing_ones() + self.chunk_bits
    }

    fn add_to_window(&mut self, new_data: &[u8]) {
        if new_data.len() < WINDOW_SIZE {
            for &b in new_data {
                debug_assert!(self.wofs < WINDOW_SIZE);
                unsafe { *self.window.get_unchecked_mut(self.wofs) = b };
                self.wofs = (self.wofs + 1) % WINDOW_SIZE;
            }
        } else {
            self.wofs = 0;
            let last_window = new_data.windows(WINDOW_SIZE).last().unwrap();
            self.window.copy_from_slice(last_window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanorand::{Rng, WyRand};

    #[test]
    fn bup_selftest() {
        use super::Bup;
        const WINDOW_SIZE: usize = 1 << 6;

        const SELFTEST_SIZE: usize = 100000;
        let mut buf = [0u8; SELFTEST_SIZE];

        fn sum(buf: &[u8]) -> u32 {
            let mut e = Bup::new();
            e.roll(buf);
            e.digest()
        }

        let mut rng = WyRand::new_seed(0x01020304);
        rng.fill_bytes(&mut buf);

        let sum1a: u32 = sum(&buf[0..]);
        let sum1b: u32 = sum(&buf[1..]);

        let sum2a: u32 =
            sum(&buf[SELFTEST_SIZE - WINDOW_SIZE * 5 / 2..SELFTEST_SIZE - WINDOW_SIZE]);
        let sum2b: u32 = sum(&buf[0..SELFTEST_SIZE - WINDOW_SIZE]);

        let sum3a: u32 = sum(&buf[0..WINDOW_SIZE + 4]);
        let sum3b: u32 = sum(&buf[3..WINDOW_SIZE + 4]);

        assert_eq!(sum1a, sum1b);
        assert_eq!(sum2a, sum2b);
        assert_eq!(sum3a, sum3b);
    }

    fn window_ordered(bup: &Bup) -> [u8; WINDOW_SIZE] {
        let mut result = bup.window;
        result.rotate_left(bup.wofs);
        result
    }

    #[test]
    fn short_no_chunk_keeps_window() {
        let data = [1, 2, 3];
        let mut bup = Bup::new();
        let edge = bup.find_chunk_edge(&data);
        assert_eq!(edge, None);
        let window = window_ordered(&bup);
        assert_eq!(&window[window.len() - 3..], &[1, 2, 3]);
        assert!(window[..window.len() - 3].iter().all(|&b| b == 0));
    }

    #[test]
    fn long_no_chunk_keeps_window() {
        let mut data = [0; 65];
        for (i, dst) in data.iter_mut().enumerate() {
            *dst = i as u8;
        }
        let mut bup = Bup::new();
        let edge = bup.find_chunk_edge(&data);
        assert_eq!(edge, None);
        let mut expected_window = [0; 64];
        for (i, dst) in expected_window.iter_mut().enumerate() {
            // Rolled over >64 bytes, will copy them starting from the beginning
            *dst = (i + 1) as u8;
        }
        assert_eq!(expected_window, window_ordered(&bup));
    }

    #[test]
    fn count_bits() {
        let bup = Bup::new_with_chunk_bits(1);
        // Ignores `chunk_bits + 1`th bit
        assert_eq!(bup.count_bits(0b001), 1);
        assert_eq!(bup.count_bits(0b011), 1);
        assert_eq!(bup.count_bits(0b101), 2);
        assert_eq!(bup.count_bits(0b111), 2);
        assert_eq!(bup.count_bits(0xFFFFFFFF), 31);

        let bup = Bup::new_with_chunk_bits(5);
        assert_eq!(bup.count_bits(0b0001011111), 6);
        assert_eq!(bup.count_bits(0b1011011111), 7);
        assert_eq!(bup.count_bits(0xFFFFFFFF), 31);
    }
}
