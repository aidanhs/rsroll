use {RollingHash, CDC};
use std::default::Default;
use std::mem;

/// Default chunk size used by `gear`
pub const CHUNK_SIZE: u32 = 1 << CHUNK_BITS;

/// Default chunk size used by `gear` (log2)
pub const CHUNK_BITS: u32 = 13;


pub struct Gear {
    digest: u64,
    pub chunk_bits: u32,
}

impl Default for Gear {
    fn default() -> Self {
        Gear {
            digest: 0,
            chunk_bits: CHUNK_BITS,
        }
    }
}


include!("_gear_rand.rs");

impl RollingHash for Gear {
    type Digest = u64;

    fn roll_byte(&mut self, b: u8) {
        self.digest = (self.digest << 1).wrapping_add(unsafe { *G.get_unchecked(b as usize) });
    }

    fn digest(&self) -> u64 {
        self.digest
    }

    fn reset(&mut self) {
        *self = Gear {
            chunk_bits: self.chunk_bits,
            .. Default::default()
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
            chunk_bits: chunk_bits,
            ..Default::default()
        }
    }
}

impl CDC for Gear {
    /// Find chunk edge using Gear defaults.
    ///
    /// See `Engine::find_chunk_edge_cond`.
    fn find_chunk<'a>(&mut self, buf: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
        const DIGEST_SIZE: usize = 64;
        debug_assert_eq!(
            mem::size_of::<<Self as RollingHash>::Digest>() * 8,
            DIGEST_SIZE
            );
        let mask = !0u64 << (DIGEST_SIZE as u32 - self.chunk_bits);

        for (i, &b) in buf.iter().enumerate() {
            self.roll_byte(b);

            if self.digest() & mask == 0 {
                self.reset();
                return Some((&buf[..i+1], &buf[i+1..]));
            }
        }
        None
    }
}


#[cfg(test)]
mod tests {
    use super::Gear;
    use {RollingHash};

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
        use test::Bencher;
        use super::*;
        use CDC;

        use tests::test_data_1mb;

        #[bench]
        fn perf_1mb(b: &mut Bencher) {
            let v = test_data_1mb();
            b.bytes = v.len() as u64;

            b.iter(|| {
                let mut cdc = Gear::new();
                let mut buf = v.as_slice();

                while let Some((_last, rest)) = cdc.find_chunk(buf) {
                    buf = rest;
                }
            });
        }
    }
}
