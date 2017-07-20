#![cfg_attr(feature = "bench", feature(test))]

#[cfg(test)]
extern crate rand;

#[cfg(feature = "bench")]
extern crate test;

/// Rolling sum and chunk splitting used by
/// `bup` - https://github.com/bup/bup/
pub mod bup;

pub mod gear;
pub use gear::Gear;

pub use bup::Bup;

/// Rolling sum engine trait
pub trait Engine {
    type Digest;

    /// Roll over one byte
    fn roll_byte(&mut self, byte: u8);

    /// Roll over a slice of bytes
    fn roll(&mut self, buf: &[u8]) {
        let _ = buf.iter().map(|&b| self.roll_byte(b)).count();
    }

    /// Return current rolling sum digest
    fn digest(&self) -> Self::Digest;

    /// Find the end of the chunk.
    ///
    /// Feed engine bytes from `buf` and stop when chunk split was found.
    ///
    /// Use `cond` function as chunk split condition.
    ///
    /// Return:
    /// None - no chunk split was found
    /// Some(offset) - offset of the first unconsumed byte of `buf`.
    ///   offset == buf.len() if the chunk ended right after whole `buf`.
    fn find_chunk_edge_cond<F>(&mut self, buf: &[u8], cond : F) -> Option<usize>
    where F : Fn(&Self) -> bool {
        for (i, &b) in buf.iter().enumerate() {
            self.roll_byte(b);

            if cond(self) {
                return Some(i + 1);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
