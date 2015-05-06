/*
 * Strongly based on
 * https://github.com/bup/bup/blob/706e8d273/lib/bup/bupsplit.c
 * https://github.com/bup/bup/blob/706e8d273/lib/bup/bupsplit.h
 * (a bit like https://godoc.org/camlistore.org/pkg/rollsum)
 */

use std::default::Default;

const BLOB_BITS: usize = 13;
const BLOB_SIZE: usize = 1 << BLOB_BITS;
const WINDOW_BITS: usize = 6;
const WINDOW_SIZE: usize = 1 << WINDOW_BITS;

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

fn rollsum_add(r: &mut Rollsum, drop: u8, add: u8) {
    r.s1 += add as usize;
    r.s1 -= drop as usize;
    r.s2 += r.s1;
    r.s2 -= WINDOW_SIZE * (drop as usize + CHAR_OFFSET);
}

pub fn rollsum_roll(r: &mut Rollsum, newch: u8) {
    // https://github.com/rust-lang/rfcs/issues/811
    let prevch = r.window[r.wofs];
    rollsum_add(r, prevch, newch);
    r.window[r.wofs] = newch;
    r.wofs = (r.wofs + 1) % WINDOW_SIZE;
}

pub fn rollsum_digest(r: &Rollsum) -> u32 {
    ((r.s1 as u32) << 16) | ((r.s2 as u32) & 0xffff)
}

pub fn rollsum_sum(buf: &[u8], ofs: usize, len: usize) -> u32 {
    let mut r: Rollsum = Default::default();
    for count in ofs..len {
        rollsum_roll(&mut r, buf[count]);
    }
    rollsum_digest(&r)
}

pub fn split_find_ofs(buf: &[u8], len: usize, bits: &mut isize) -> isize {
    let mut r: Rollsum = Default::default();
    for count in 0..len {
        rollsum_roll(&mut r, buf[count]);
        if r.s2 & (BLOB_SIZE - 1) == (!0) & (BLOB_SIZE - 1) {
            let mut rsum: u32 = rollsum_digest(&r) >> BLOB_BITS;
            *bits = BLOB_BITS as isize;
            loop {
                rsum >>= 1;
                if (rsum & 1) == 0 {
                    break;
                }
                (*bits) += 1;
            }
            return count as isize + 1;
        }
    }
    return 0;
}


extern crate rand;
#[allow(unused_imports)]
use rand::{Rng, SeedableRng, StdRng};
#[test]
fn bupsplit_selftest()
{
    const SELFTEST_SIZE: usize = 100000;
    let mut buf = [0u8; SELFTEST_SIZE];

    let seed: &[_] = &[1, 2, 3, 4];
    let mut rng: StdRng = SeedableRng::from_seed(seed);
    for count in 0..SELFTEST_SIZE {
        buf[count] = rng.gen();
    }

    let sum1a: u32 = rollsum_sum(&buf, 0, SELFTEST_SIZE);
    let sum1b: u32 = rollsum_sum(&buf, 1, SELFTEST_SIZE);
    let sum2a: u32 = rollsum_sum(&buf, SELFTEST_SIZE - WINDOW_SIZE*5/2,
                     SELFTEST_SIZE - WINDOW_SIZE);
    let sum2b: u32 = rollsum_sum(&buf, 0, SELFTEST_SIZE - WINDOW_SIZE);
    let sum3a: u32 = rollsum_sum(&buf, 0, WINDOW_SIZE+3);
    let sum3b: u32 = rollsum_sum(&buf, 3, WINDOW_SIZE+3);

    println!("sum1a = {}\n", sum1a);
    println!("sum1b = {}\n", sum1b);
    println!("sum2a = {}\n", sum2a);
    println!("sum2b = {}\n", sum2b);
    println!("sum3a = {}\n", sum3a);
    println!("sum3b = {}\n", sum3b);

    if sum1a != sum1b || sum2a != sum2b || sum3a != sum3b {
        panic!("fail");
    }
}
