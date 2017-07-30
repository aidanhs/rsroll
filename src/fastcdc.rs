use super::Engine;
use std::default::Default;
use std::cmp;
use std::mem;
use Gear;

pub struct FastCDC {
    current_chunk_size: u64,
    gear: Gear,
}

impl Default for FastCDC {
    fn default() -> Self {
        FastCDC {
            current_chunk_size: 0,
            gear: Gear::default(),
        }
    }
}


impl Engine for FastCDC {
    type Digest = u64;

    #[inline(always)]
    fn roll_byte(&mut self, b: u8) {
        self.gear.roll_byte(b);
    }

    #[inline(always)]
    fn digest(&self) -> u64 {
        self.gear.digest()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.gear.reset();
        self.current_chunk_size = 0;
    }
}

impl FastCDC {
    /// Create new FastCDC engine with default chunking settings
    pub fn new() -> Self {
        Default::default()
    }

    /// Create new Gear engine with custom chunking settings
    ///
    /// `chunk_bits` is number of bits that need to match in
    /// the edge condition. `CHUNK_BITS` constant is the default.
    pub fn new_with_chunk_bits(chunk_bits: u32) -> Self {
        Self {
            current_chunk_size: 0,
            gear: Gear::new_with_chunk_bits(chunk_bits),
        }
    }

    /// Find chunk edge using Gear defaults.
    ///
    /// See `Engine::find_chunk_edge_cond`.
    pub fn find_chunk_edge(&mut self, mut buf: &[u8]) -> Option<(usize, u64)> {

        const DIGEST_SIZE: usize = 64;
        debug_assert_eq!(
            mem::size_of::<<Self as Engine>::Digest>() * 8,
            DIGEST_SIZE
            );

        const SPREAD_BITS: u32 = 3;
        const WINDOW_SIZE: usize = 64;

        let min_shift = DIGEST_SIZE as u32 - self.gear.chunk_bits - SPREAD_BITS;
        let max_shift = DIGEST_SIZE as u32 - self.gear.chunk_bits + SPREAD_BITS;
        let min_size = (1 << (self.gear.chunk_bits - SPREAD_BITS)) as u64;
        let ignore_size = min_size - WINDOW_SIZE as u64;
        let avg_size = (1 << self.gear.chunk_bits) as u64;
        let max_size = (1 << (self.gear.chunk_bits + SPREAD_BITS)) as u64;

        let mut cur_offset = 0usize;

        loop {
            debug_assert!(self.current_chunk_size < max_size);
            debug_assert!(cur_offset < max_size as usize);

            if buf.is_empty() {
                return None
            }

            // ignore bytes that are not going to influence the digest
            if self.current_chunk_size < ignore_size {
                let skip_bytes = cmp::min(ignore_size  - self.current_chunk_size, buf.len() as u64);
                self.current_chunk_size += skip_bytes;
                cur_offset += skip_bytes as usize;
                buf = &buf[skip_bytes as usize..];
            }

            // ignore edges in bytes that are smaller than min_size
            if self.current_chunk_size < min_size {
                let roll_bytes = cmp::min(min_size - self.current_chunk_size,
                                          buf.len() as u64);
                self.gear.roll(&buf[..roll_bytes as usize]);
                self.current_chunk_size += roll_bytes;
                cur_offset += roll_bytes as usize;
                buf = &buf[roll_bytes as usize..];
            }

            // roll through early bytes with smaller probability
            if self.current_chunk_size < avg_size {
                let roll_bytes = cmp::min(avg_size - self.current_chunk_size,
                                          buf.len() as u64);
                let result = self.gear.find_chunk_edge_cond(buf, |e: &Gear| (e.digest() >> min_shift) == 0);

                if let Some((offset, digest)) = result {
                    self.reset();
                    return Some((cur_offset + offset, digest));
                }

                self.current_chunk_size += roll_bytes;
                cur_offset += roll_bytes as usize;
                buf = &buf[roll_bytes as usize..];
            }

            // roll through late bytes with higher probability
            if self.current_chunk_size < max_size {
                let roll_bytes = cmp::min(max_size - self.current_chunk_size,
                                          buf.len() as u64);
                let result = self.gear.find_chunk_edge_cond(buf, |e: &Gear| (e.digest() >> max_shift) == 0);

                if let Some((offset, digest)) = result {
                    self.reset();
                    return Some((cur_offset + offset, digest));
                }

                self.current_chunk_size += roll_bytes;
                cur_offset += roll_bytes as usize;
                buf = &buf[roll_bytes as usize..];
            }

            if self.current_chunk_size >= max_size {
                debug_assert_eq!(self.current_chunk_size, max_size);
                let result = (cur_offset, self.gear.digest());
                self.reset();
                return Some(result);
            }

        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FastCDC, Engine};

    #[test]
    fn effective_window_size() {
        let ones = vec![0x1; 1024];
        let zeroes = vec![0x0; 1024];

        let mut gear = FastCDC::new();
        gear.roll(&ones);
        let digest = gear.digest();

        let mut gear = FastCDC::new();
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

        use tests::test_data_1mb;

        #[bench]
        fn perf_1mb(b: &mut Bencher) {
            let v = test_data_1mb();

            b.iter(|| {
                let mut gear = FastCDC::new();
                let mut i = 0;
                while let Some((new_i, _)) = gear.find_chunk_edge(&v[i..v.len()]) {
                    i += new_i;
                    if i == v.len() {
                        break;
                    }
                }
            });
        }

        #[bench]
        fn perf_1mb_16k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();

            b.iter(|| {
                let mut gear = FastCDC::new_with_chunk_bits(14);
                let mut i = 0;
                while let Some((new_i, _)) = gear.find_chunk_edge(&v[i..v.len()]) {
                    i += new_i;
                    if i == v.len() {
                        break;
                    }
                }
            });
        }
        #[bench]
        fn perf_1mb_64k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();

            b.iter(|| {
                let mut gear = FastCDC::new_with_chunk_bits(16);
                let mut i = 0;
                while let Some((new_i, _)) = gear.find_chunk_edge(&v[i..v.len()]) {
                    i += new_i;
                    if i == v.len() {
                        break;
                    }
                }
            });
        }

        #[bench]
        fn perf_1mb_128k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();

            b.iter(|| {
                let mut gear = FastCDC::new_with_chunk_bits(17);
                let mut i = 0;
                while let Some((new_i, _)) = gear.find_chunk_edge(&v[i..v.len()]) {
                    i += new_i;
                    if i == v.len() {
                        break;
                    }
                }
            });
        }


        #[bench]
        fn perf_1mb_256k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();

            b.iter(|| {
                let mut gear = FastCDC::new_with_chunk_bits(18);
                let mut i = 0;
                while let Some((new_i, _)) = gear.find_chunk_edge(&v[i..v.len()]) {
                    i += new_i;
                    if i == v.len() {
                        break;
                    }
                }
            });
        }

        #[bench]
        fn perf_1mb_512k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();

            b.iter(|| {
                let mut gear = FastCDC::new_with_chunk_bits(19);
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
