#![cfg_attr(feature = "bench", feature(test))]

#[cfg(feature = "bench")]
extern crate test;

/// Rolling sum and chunk splitting used by
/// `bup` - https://github.com/bup/bup/
#[cfg(feature = "bup")]
pub mod bup;
#[cfg(feature = "bup")]
pub use crate::bup::Bup;

#[cfg(feature = "gear")]
pub mod gear;
#[cfg(feature = "gear")]
pub use crate::gear::Gear;

/// Rolling sum engine trait
pub trait Engine {
    type Digest;

    /// Roll over one byte
    fn roll_byte(&mut self, byte: u8);

    /// Roll over a slice of bytes
    fn roll(&mut self, buf: &[u8]) {
        buf.iter().for_each(|&b| self.roll_byte(b));
    }

    /// Return current rolling sum digest
    fn digest(&self) -> Self::Digest;

    /// Resets the internal state
    fn reset(&mut self);

    /// Find the end of the chunk.
    ///
    /// Feed engine bytes from `buf` and stop when chunk split was found.
    ///
    /// Use `cond` function as chunk split condition.
    ///
    /// When edge is find, state of `self` is reset, using `reset()` method.
    ///
    /// Returns:
    ///
    /// * None - no chunk split was found
    /// * Some - offset of the first unconsumed byte of `buf` and the digest of
    ///   the whole chunk. `offset` == buf.len() if the chunk ended right after
    ///   the whole `buf`.
    fn find_chunk_edge_cond<F>(&mut self, buf: &[u8], cond: F) -> Option<(usize, Self::Digest)>
    where
        F: Fn(&Self) -> bool,
    {
        for (i, &b) in buf.iter().enumerate() {
            self.roll_byte(b);

            if cond(self) {
                let digest = self.digest();
                self.reset();
                return Some((i + 1, digest));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
