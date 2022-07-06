use super::Engine;
use std::default::Default;
use std::mem;
use std::num::Wrapping;

pub type Digest = u64;

/// Default chunk size used by `gear`
pub const CHUNK_SIZE: u32 = 1 << CHUNK_BITS;

/// Default chunk size used by `gear` (log2)
pub const CHUNK_BITS: u32 = 13;

/// The effective window size used by `gear`
pub const WINDOW_SIZE: usize = mem::size_of::<Digest>() * 8;

pub struct Gear {
    digest: Wrapping<Digest>,
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
    type Digest = Digest;

    #[inline(always)]
    fn roll_byte(&mut self, b: u8) {
        self.digest <<= 1;
        self.digest += Wrapping(G[b as usize]);
    }

    fn roll(&mut self, buf: &[u8]) {
        crate::roll_windowed(self, WINDOW_SIZE, buf);
    }

    #[inline(always)]
    fn digest(&self) -> Digest {
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
    pub fn find_chunk_edge(&mut self, buf: &[u8]) -> Option<(usize, Digest)> {
        const DIGEST_SIZE: usize = mem::size_of::<Digest>() * 8;
        let shift = DIGEST_SIZE as u32 - self.chunk_bits;
        self.find_chunk_edge_cond(buf, |e: &Gear| (e.digest() >> shift) == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rand_data;

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
            if gear.digest() == digest {
                assert_eq!(i, WINDOW_SIZE);
                return;
            }
            gear.roll_byte(b);
        }

        panic!("matching digest not found");
    }

    #[test]
    fn edge_expected_size() {
        let data = rand_data(2 * 1024 * 1024);
        for bits in 4..13 {
            let mut gear = Gear::new_with_chunk_bits(bits);
            let mut size_count = 0;
            let mut total_sizes = 0;
            let mut remaining = &data[..];
            while let Some((i, _)) = gear.find_chunk_edge(remaining) {
                size_count += 1;
                total_sizes += i;
                remaining = &remaining[i..];
            }

            let expected_average = (1 << bits) as f64;
            let average = total_sizes as f64 / size_count as f64;
            assert!(dbg!((average - expected_average).abs() / expected_average) < 0.1)
        }
    }
}
