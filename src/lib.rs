/*
 * Strongly based on
 * https://github.com/bup/bup/blob/706e8d273/lib/bup/bupsplit.c
 * https://github.com/bup/bup/blob/706e8d273/lib/bup/bupsplit.h
 * (a bit like https://godoc.org/camlistore.org/pkg/rollsum)
 */

#[cfg(test)]
extern crate rand;

use std::default::Default;

const BLOB_BITS: usize = 13;
const BLOB_SIZE: usize = 1 << BLOB_BITS;
const WINDOW_BITS: usize = 6;
pub const WINDOW_SIZE: usize = 1 << WINDOW_BITS;

const CHAR_OFFSET: usize = 31;

pub struct Rollsum {
    pub s1: usize,
    pub s2: usize,
    pub window: [u8; WINDOW_SIZE],
    pub wofs: usize,
}

impl Default for Rollsum {
    fn default() -> Rollsum {
        Rollsum {
            s1: WINDOW_SIZE * CHAR_OFFSET,
            s2: WINDOW_SIZE * (WINDOW_SIZE-1) * CHAR_OFFSET,
            window: [0; WINDOW_SIZE],
            wofs: 0,
        }
    }
}

impl Rollsum {
    fn add(&mut self, drop: u8, add: u8) {
        self.s1 += add as usize;
        self.s1 -= drop as usize;
        self.s2 += self.s1;
        self.s2 -= WINDOW_SIZE * (drop as usize + CHAR_OFFSET);
    }
    pub fn roll(&mut self, newch: u8) {
        // https://github.com/rust-lang/rfcs/issues/811
        let prevch = self.window[self.wofs];
        self.add(prevch, newch);
        self.window[self.wofs] = newch;
        self.wofs = (self.wofs + 1) % WINDOW_SIZE;
    }
    pub fn digest(&self) -> u32 {
        ((self.s1 as u32) << 16) | ((self.s2 as u32) & 0xffff)
    }
}

pub fn rollsum_sum(buf: &[u8], ofs: usize, len: usize) -> u32 {
    let mut rs: Rollsum = Default::default();
    for count in ofs..len {
        rs.roll(buf[count]);
    }
    rs.digest()
}

pub fn split_find_ofs(buf: &[u8]) -> (usize, isize) {
    let mut bits: isize = -1;
    let mut rs: Rollsum = Default::default();
    for count in 0..buf.len() {
        rs.roll(buf[count]);
        if !(rs.s2 & (BLOB_SIZE - 1) == (!0) & (BLOB_SIZE - 1)) {
            continue;
        }
        let mut rsum: u32 = rs.digest() >> BLOB_BITS;
        bits = BLOB_BITS as isize;
        loop {
            rsum >>= 1;
            if (rsum & 1) == 0 {
                break;
            }
            bits += 1;
        }
        return (count + 1, bits);
    }
    return (0, bits);
}

#[cfg(test)]
mod tests;
