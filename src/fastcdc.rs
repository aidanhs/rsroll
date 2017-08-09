use super::{RollingHash, CDC};
use std::default::Default;
use std::{cmp, mem};
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

/// FastCDC chunking
///
/// * Paper: "FastCDC: a Fast and Efficient Content-Defined Chunking Approach for Data Deduplication"
/// * Paper-URL: https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf
/// * Presentation: https://www.usenix.org/sites/default/files/conference/protected-files/atc16_slides_xia.pdf
pub struct FastCDC {
    current_chunk_size: u64,
    gear: Gear,
    mask_short: u64,
    mask_long: u64,
    ignore_size: u64,
    min_size: u64,
    avg_size: u64,
    max_size: u64,
}

impl Default for FastCDC {
    fn default() -> Self {
        FastCDC::new()
    }
}

impl FastCDC {
    /// Create new FastCDC engine with default chunking settings
    pub fn new() -> Self {
        FastCDC::new_with_chunk_bits(gear::CHUNK_BITS)
    }

    fn reset(&mut self) {
        self.gear.reset();
        self.current_chunk_size = 0;
    }

    /// Create new `FastCDC` engine with custom chunking settings
    ///
    /// `chunk_bits` is number of bits that need to match in
    /// the edge condition. `CHUNK_BITS` constant is the default.
    pub fn new_with_chunk_bits(chunk_bits: u32) -> Self {
        let (mask_short, mask_long) = get_masks(1 << chunk_bits, 2, 0);
        let gear = Gear::new_with_chunk_bits(chunk_bits);
        const DIGEST_SIZE: usize = 64;
        debug_assert_eq!(
            mem::size_of::<<Gear as RollingHash>::Digest>() * 8,
            DIGEST_SIZE
        );

        const SPREAD_BITS: u32 = 3;
        const WINDOW_SIZE: usize = 64;

        let min_size = (1 << (gear.chunk_bits - SPREAD_BITS + 1)) as u64;

        let ignore_size = min_size - WINDOW_SIZE as u64;
        let avg_size = (1 << gear.chunk_bits) as u64;
        let max_size = (1 << (gear.chunk_bits + SPREAD_BITS)) as u64;

        Self {
            current_chunk_size: 0,
            gear: gear,
            mask_short: mask_short,
            mask_long: mask_long,
            ignore_size: ignore_size,
            min_size: min_size,
            avg_size: avg_size,
            max_size: max_size,
        }
    }
}

impl CDC for FastCDC {
    /// Find chunk edge using `FastCDC` defaults.
    fn find_chunk<'a>(&mut self, whole_buf: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
        let mut buf = whole_buf;

        debug_assert!(self.current_chunk_size < self.max_size);

        // ignore bytes that are not going to influence the digest
        if self.current_chunk_size < self.ignore_size {
            let skip_bytes = cmp::min(self.ignore_size - self.current_chunk_size, buf.len() as u64);
            self.current_chunk_size += skip_bytes;
            buf = &buf[skip_bytes as usize..];
        }

        // ignore edges in bytes that are smaller than min_size
        if self.current_chunk_size < self.min_size {
            let roll_bytes = cmp::min(self.min_size - self.current_chunk_size, buf.len() as u64);
            self.gear.roll(&buf[..roll_bytes as usize]);
            self.current_chunk_size += roll_bytes;
            buf = &buf[roll_bytes as usize..];
        }

        // roll through early bytes with smaller probability
        if self.current_chunk_size < self.avg_size {
            let roll_bytes = cmp::min(self.avg_size - self.current_chunk_size, buf.len() as u64);
            let result = self.gear.find_chunk_mask(buf, self.mask_short);

            if let Some(result) = result {
                self.reset();
                return Some(result);
            }

            self.current_chunk_size += roll_bytes;
            buf = &buf[roll_bytes as usize..];
        }

        // roll through late bytes with higher probability
        if self.current_chunk_size < self.max_size {
            let roll_bytes = cmp::min(self.max_size - self.current_chunk_size, buf.len() as u64);
            let result = self.gear.find_chunk_mask(buf, self.mask_long);

            if let Some(result) = result {
                self.reset();
                return Some(result);
            }

            self.current_chunk_size += roll_bytes;
            buf = &buf[roll_bytes as usize..];
        }

        if self.current_chunk_size >= self.max_size {
            debug_assert_eq!(self.current_chunk_size, self.max_size);
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

#[cfg(test)]
mod tests {

    #[cfg(feature = "bench")]
    mod bench {
        use test::Bencher;
        use super::*;

        use tests::test_data_1mb;

        use CDC;

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
        fn perf_1mb_128k_chunks(b: &mut Bencher) {
            let v = test_data_1mb();
            b.bytes = v.len() as u64;

            b.iter(|| {
                let mut cdc = FastCDC::new_with_chunk_bits(17);
                let mut buf = v.as_slice();

                while let Some((_last, rest)) = cdc.find_chunk(buf) {
                    buf = rest;
                }
            });
        }
    }
}
