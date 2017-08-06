#![cfg_attr(feature = "bench", feature(test))]

#[cfg(test)]
extern crate rand;

#[cfg(feature = "bench")]
extern crate test;

/// Rolling sum and chunk splitting used by
/// `bup` - https://github.com/bup/bup/
pub mod bup;
pub use bup::Bup;

pub mod gear;
pub use gear::Gear;

pub mod fastcdc;
pub use fastcdc::FastCDC;

/// Rolling sum engine trait
pub trait RollingHash {
    type Digest;

    fn roll_byte(&mut self, buf: u8);

    /// Roll over a slice of bytes
    fn roll(&mut self, buf: &[u8]) {
        buf.iter().map(|&b| self.roll_byte(b)).count();
    }

    /// Return current rolling sum digest
    fn digest(&self) -> Self::Digest;

    /// Resets the internal state
    fn reset(&mut self);
}

trait CDC {
    /// Find the end of the chunk.
    ///
    /// When edge is find, state of CDC should automatically be reset.
    ///
    /// Returns:
    ///
    /// * None - no chunk split was found, and the whole `buf` belongs
    ///          to the current chunk.
    /// * Some - chunk edge was found, and it is splitting the `buf`
    ///          in two pieces. The second one has not yet been searched
    ///          for more chunks.
    fn find_chunk<'a>(&mut self, buf: &'a [u8]) -> Option<(&'a [u8], &'a [u8])>;

}

#[cfg(test)]
mod tests;
