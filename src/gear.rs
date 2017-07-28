use super::Engine;
use std::default::Default;
use std::num::Wrapping;

/// Default chunk size used by `gear`
pub const CHUNK_SIZE: u32 = 1 << CHUNK_BITS;

/// Default chunk size used by `gear` (log2)
pub const CHUNK_BITS: u32 = 13;


pub struct Gear {
    digest: Wrapping<u32>,
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
    type Digest = u32;

    #[inline(always)]
    fn roll_byte(&mut self, b: u8) {
        self.digest = self.digest << 1;
        self.digest += unsafe { Wrapping(*G.get_unchecked(b as usize)) };
    }

    #[inline(always)]
    fn digest(&self) -> u32 {
        self.digest.0
    }

    #[inline(always)]
    fn reset(&mut self) {
        let chunk_bits = self.chunk_bits;
        *self = Default::default();
        self.chunk_bits = chunk_bits;
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
            chunk_bits: chunk_bits,
            ..Default::default()
        }
    }

    /// Find chunk edge using Gear defaults.
    ///
    /// See `Engine::find_chunk_edge_cond`.
    pub fn find_chunk_edge(&mut self, buf: &[u8]) -> Option<(usize, u32)> {
        let shift  = 32 - self.chunk_bits;
        self.find_chunk_edge_cond(buf, |e: &Gear| (e.digest() >> shift) == 0)
    }
}

#[cfg(feature = "bench")]
mod tests {
    use test::Bencher;
    use super::Gear;
    use rand::{Rng, SeedableRng, StdRng};

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
