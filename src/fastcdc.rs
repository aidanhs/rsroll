use super::{RollingHash, CDC};
use std::default::Default;
use std::cmp;
use std::mem;
use {gear, Gear};

fn get_masks(avg_size: usize, nc_level: usize, seed: u64) -> (u64, u64) {
    let bits = (avg_size.next_power_of_two() - 1).count_ones();
    if bits == 13 {
        // From the paper
        return (0x0003590703530000, 0x0000d90003530000);
    }
    let mut mask = 0u64;
    let mut v = seed;
    let a = 6364136223846793005;
    let c = 1442695040888963407;
    while mask.count_ones() < bits - nc_level as u32 {
        v = v.wrapping_mul(a).wrapping_add(c);
        mask = (mask | 1).rotate_left(v as u32 & 0x3f);
    }
    let mask_long = mask;
    while mask.count_ones() < bits + nc_level as u32 {
        v = v.wrapping_mul(a).wrapping_add(c);
        mask = (mask | 1).rotate_left(v as u32 & 0x3f);
    }
    let mask_short = mask;
    (mask_short, mask_long)
}

pub struct FastCDC {
    current_chunk_size: u64,
    gear: Gear,
    mask_long: u64,
    mask_short: u64,
}

impl Default for FastCDC {
    fn default() -> Self {
        let (mask_short, mask_long) = get_masks(1 << gear::CHUNK_BITS, 2, 0);
        FastCDC {
            current_chunk_size: 0,
            gear: Gear::default(),
            mask_short: mask_short,
            mask_long: mask_long,
        }
    }
}


impl RollingHash for FastCDC {
    type Digest = u64;

    fn roll_byte(&mut self, b: u8) {
        self.gear.roll_byte(b);
    }

    fn digest(&self) -> u64 {
        self.gear.digest()
    }

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

    /// Create new `FastCDC` engine with custom chunking settings
    ///
    /// `chunk_bits` is number of bits that need to match in
    /// the edge condition. `CHUNK_BITS` constant is the default.
    pub fn new_with_chunk_bits(chunk_bits: u32) -> Self {
        let (mask_short, mask_long) = get_masks(1 << chunk_bits, 2, 0);
        Self {
            current_chunk_size: 0,
            gear: Gear::new_with_chunk_bits(chunk_bits),
            mask_short: mask_short,
            mask_long: mask_long,
        }
    }
}

impl CDC for FastCDC {
    /// Find chunk edge using `FastCDC` defaults.
    ///
    /// See `RollingHash::find_chunk_edge_cond`.
    fn find_chunk<'a>(&mut self, whole_buf: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {

        const DIGEST_SIZE: usize = 64;
        debug_assert_eq!(
            mem::size_of::<<Self as RollingHash>::Digest>() * 8,
            DIGEST_SIZE
        );

        const SPREAD_BITS: u32 = 3;
        const WINDOW_SIZE: usize = 64;

        let min_mask = self.mask_short;
        let max_mask = self.mask_long;

        let min_size = (1 << (self.gear.chunk_bits - SPREAD_BITS + 1)) as u64;

        let ignore_size = min_size - WINDOW_SIZE as u64;
        let avg_size = (1 << self.gear.chunk_bits) as u64;
        let max_size = (1 << (self.gear.chunk_bits + SPREAD_BITS)) as u64;

        let mut buf = whole_buf;

        loop {
            debug_assert!(self.current_chunk_size < max_size);


            // ignore bytes that are not going to influence the digest
            if self.current_chunk_size < ignore_size {
                let skip_bytes = cmp::min(ignore_size - self.current_chunk_size, buf.len() as u64);
                self.current_chunk_size += skip_bytes;
                buf = &buf[skip_bytes as usize..];
            }

            // ignore edges in bytes that are smaller than min_size
            if self.current_chunk_size < min_size {
                let roll_bytes = cmp::min(min_size - self.current_chunk_size, buf.len() as u64);
                self.gear.roll(&buf[..roll_bytes as usize]);
                self.current_chunk_size += roll_bytes;
                buf = &buf[roll_bytes as usize..];
            }

            // roll through early bytes with smaller probability
            if self.current_chunk_size < avg_size {
                let roll_bytes = cmp::min(avg_size - self.current_chunk_size, buf.len() as u64);
                let result = self.gear.find_chunk_mask(buf, min_mask);

                if let Some(result) = result {
                    self.reset();
                    return Some(result);
                }

                self.current_chunk_size += roll_bytes;
                buf = &buf[roll_bytes as usize..];
            }

            // roll through late bytes with higher probability
            if self.current_chunk_size < max_size {
                let roll_bytes = cmp::min(max_size - self.current_chunk_size, buf.len() as u64);
                let result = self.gear.find_chunk_mask(buf, max_mask);

                if let Some(result) = result {
                    self.reset();
                    return Some(result);
                }

                self.current_chunk_size += roll_bytes;
                buf = &buf[roll_bytes as usize..];
            }

            if self.current_chunk_size >= max_size {
                debug_assert_eq!(self.current_chunk_size, max_size);
                //let result = (&whole_buf[..cur_offset+1], &whole_buf[cur_offset+1..]);
                let result = (&whole_buf[..whole_buf.len() - buf.len()], buf);
                self.reset();
                return Some(result);
            }

            if buf.is_empty() {
                return None;
            }
            unreachable!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FastCDC, RollingHash};

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

        use CDC;

        #[bench]
        fn perf_1mb_512k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();
            b.bytes = v.len() as u64;

            b.iter(|| {
                let mut cdc = FastCDC::new_with_chunk_bits(19);
                let mut buf = v.as_slice();

                while let Some((_last, rest)) = cdc.find_chunk(buf) {
                    buf = rest;
                }
            });
        }
        #[bench]
        fn perf_1mb_064k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();
            b.bytes = v.len() as u64;

            b.iter(|| {
                let mut cdc = FastCDC::new_with_chunk_bits(16);
                let mut buf = v.as_slice();

                while let Some((_last, rest)) = cdc.find_chunk(buf) {
                    buf = rest;
                }
            });
        }

        #[bench]
        fn perf_1mb_008k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();
            b.bytes = v.len() as u64;

            b.iter(|| {
                let mut cdc = FastCDC::new_with_chunk_bits(13);
                let mut buf = v.as_slice();

                while let Some((_last, rest)) = cdc.find_chunk(buf) {
                    buf = rest;
                }
            });
        }
        #[bench]
        fn perf_1mb_004k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();
            b.bytes = v.len() as u64;

            b.iter(|| {
                let mut cdc = FastCDC::new_with_chunk_bits(12);
                let mut buf = v.as_slice();

                while let Some((_last, rest)) = cdc.find_chunk(buf) {
                    buf = rest;
                }
            });
        }
    }
}
