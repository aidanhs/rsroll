use super::Engine;
use std::default::Default;

const WINDOW_BITS: usize = 6;
const WINDOW_SIZE: usize = 1 << WINDOW_BITS;

const CHAR_OFFSET: usize = 31;

/// Default chunk size used by `bup`
pub const CHUNK_SIZE: u32 = 1 << CHUNK_SIZE_LOG2;

/// Default chunk size used by `bup` (log2)
pub const CHUNK_SIZE_LOG2: u32 = 13;


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
}

impl Default for Bup {
    fn default() -> Self {
        Bup {
            s1: WINDOW_SIZE * CHAR_OFFSET,
            s2: WINDOW_SIZE * (WINDOW_SIZE-1) * CHAR_OFFSET,
            window: [0; WINDOW_SIZE],
            wofs: 0,
        }
    }
}


impl Engine for Bup {
    type Digest = u32;

    fn roll_byte(&mut self, newch: u8) {
        // https://github.com/rust-lang/rfcs/issues/811
        let prevch = self.window[self.wofs];
        self.add(prevch, newch);
        self.window[self.wofs] = newch;
        self.wofs = (self.wofs + 1) % WINDOW_SIZE;
    }

    fn digest(&self) -> u32 {
        ((self.s1 as u32) << 16) | ((self.s2 as u32) & 0xffff)
    }
}

impl Bup {
    /// Create new Bup engine
    pub fn new() -> Self {
        Default::default()
    }

    fn add(&mut self, drop: u8, add: u8) {
        self.s1 += add as usize;
        self.s1 -= drop as usize;
        self.s2 += self.s1;
        self.s2 -= WINDOW_SIZE * (drop as usize + CHAR_OFFSET);
    }

    /// Find chunk edge using Bup defaults.
    ///
    /// See `Engine::find_chunk_edge_cond`.
    pub fn find_chunk_edge<F>(&mut self, buf: &[u8]) -> Option<usize> {
        self.find_chunk_edge_cond(buf, |e : &Bup |
            (e.digest() & (CHUNK_SIZE - 1)) == (CHUNK_SIZE - 1)
        )
    }
}
