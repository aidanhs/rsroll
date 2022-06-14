use super::Engine;
use std::default::Default;

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
    s1: usize,
    s2: usize,
    window: [u8; WINDOW_SIZE],
    wofs: usize,
    chunk_bits: u32,
}

impl Default for Bup {
    fn default() -> Self {
        Bup {
            s1: WINDOW_SIZE * CHAR_OFFSET,
            s2: WINDOW_SIZE * (WINDOW_SIZE - 1) * CHAR_OFFSET,
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
        // https://github.com/rust-lang/rfcs/issues/811
        let prevch = unsafe { *self.window.get_unchecked(self.wofs) };
        self.add(prevch, newch);
        unsafe { *self.window.get_unchecked_mut(self.wofs) = newch };
        self.wofs = (self.wofs + 1) % WINDOW_SIZE;
    }

    fn roll(&mut self, buf: &[u8]) {
        crate::roll_windowed(self, WINDOW_SIZE, buf);
    }

    #[inline(always)]
    fn digest(&self) -> Digest {
        ((self.s1 as Digest) << 16) | ((self.s2 as Digest) & 0xffff)
    }

    #[inline]
    fn reset(&mut self) {
        *self = Bup {
            chunk_bits: self.chunk_bits,
            ..Default::default()
        }
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

    #[inline(always)]
    fn add(&mut self, drop: u8, add: u8) {
        self.s1 += add as usize;
        self.s1 -= drop as usize;
        self.s2 += self.s1;
        self.s2 -= WINDOW_SIZE * (drop as usize + CHAR_OFFSET);
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
