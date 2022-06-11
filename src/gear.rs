use super::Engine;
use std::default::Default;
use std::mem;
use std::num::Wrapping;

/// Default chunk size used by `gear`
pub const CHUNK_SIZE: u32 = 1 << CHUNK_BITS;

/// Default chunk size used by `gear` (log2)
pub const CHUNK_BITS: u32 = 13;

pub struct Gear {
    digest: Wrapping<u64>,
    chunk_bits: u32,
}

impl Default for Gear {
    fn default() -> Self {
        Gear {
            digest: Wrapping(0),
            chunk_bits: CHUNK_BITS,
        }
    }
}

include!("_gear_rand.rs");

impl Engine for Gear {
    type Digest = u64;

    #[inline(always)]
    fn roll_byte(&mut self, b: u8) {
        self.digest <<= 1;
        self.digest += Wrapping(unsafe { *G.get_unchecked(b as usize) });
    }

    #[inline(always)]
    fn digest(&self) -> u64 {
        self.digest.0
    }

    #[inline]
    fn reset(&mut self) {
        *self = Gear {
            chunk_bits: self.chunk_bits,
            ..Default::default()
        }
    }
}

impl Gear {
    /// Create new Gear engine with default chunking settings
    pub fn new() -> Self {
        Default::default()
    }

    /// Create new Gear engine with custom chunking settings
    ///
    /// `chunk_bits` is number of bits that need to match in
    /// the edge condition. `CHUNK_BITS` constant is the default.
    pub fn new_with_chunk_bits(chunk_bits: u32) -> Self {
        assert!(chunk_bits < 32);
        Gear {
            chunk_bits,
            ..Default::default()
        }
    }

    /// Find chunk edge using Gear defaults.
    ///
    /// See `Engine::find_chunk_edge_cond`.
    pub fn find_chunk_edge(&mut self, buf: &[u8]) -> Option<(usize, u64)> {
        const DIGEST_SIZE: usize = 64;
        debug_assert_eq!(mem::size_of::<<Self as Engine>::Digest>() * 8, DIGEST_SIZE);
        let shift = DIGEST_SIZE as u32 - self.chunk_bits;
        self.find_chunk_edge_cond(buf, |e: &Gear| (e.digest() >> shift) == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, Gear};

    #[test]
    fn effective_window_size() {
        let ones = vec![0x1; 1024];
        let zeroes = vec![0x0; 1024];

        let mut gear = Gear::new();
        gear.roll(&ones);
        let digest = gear.digest();

        let mut gear = Gear::new();
        gear.roll(&zeroes);

        for (i, &b) in ones.iter().enumerate() {
            gear.roll_byte(b);
            if gear.digest() == digest {
                assert_eq!(i, 63);
                return;
            }
        }

        panic!("matching digest not found");
    }

    #[cfg(feature = "bench")]
    mod bench {
        use super::*;
        use rand::{Rng, SeedableRng, StdRng};
        use test::Bencher;

        #[bench]
        fn gear_perf_1mb(b: &mut Bencher) {
            let mut v = vec![0x0; 1024 * 1024];

            let seed: &[_] = &[1, 2, 3, 4];
            let mut rng: StdRng = SeedableRng::from_seed(seed);
            for i in 0..v.len() {
                v[i] = rng.gen();
            }

            b.iter(|| {
                let mut gear = Gear::new();
                let mut i = 0;
                while let Some((new_i, _)) = gear.find_chunk_edge(&v[i..v.len()]) {
                    i += new_i;
                    if i == v.len() {
                        break;
                    }
                }
            });
        }
    }
}
