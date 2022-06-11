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
mod tests {
    use super::*;
    use nanorand::{Rng, WyRand};

    fn rand_data(len: usize) -> Vec<u8> {
        let mut data = vec![0; len];
        let mut rng = WyRand::new_seed(0x01020304);
        rng.fill_bytes(&mut data);
        data
    }

    macro_rules! test_engine {
        ($name:ident, $engine:ty) => {
            mod $name {
                use super::*;

                #[test]
                fn roll_byte_same_as_roll() {
                    let mut engine1 = <$engine>::default();
                    let mut engine2 = <$engine>::default();

                    let data = rand_data(1024);
                    for (i, &b) in data.iter().enumerate() {
                        engine1.roll_byte(b);

                        engine2.reset();
                        engine2.roll(&data[..=i]);
                        assert_eq!(engine1.digest(), engine2.digest());

                        let mut engine3 = <$engine>::default();
                        engine3.roll(&data[..=i]);
                        assert_eq!(engine1.digest(), engine3.digest());
                    }
                }

                #[test]
                fn chunk_edge_correct_digest() {
                    let mut engine1 = <$engine>::default();

                    let data = rand_data(512 * 1024);
                    let mut remaining = &data[..];
                    while let Some((i, digest)) =
                        engine1.find_chunk_edge_cond(remaining, |e| e.digest() & 0x0F == 0x0F)
                    {
                        let mut engine2 = <$engine>::default();
                        engine2.roll(&remaining[..i]);
                        assert_eq!(engine2.digest(), digest);

                        remaining = &remaining[i..];
                        engine2.reset();
                        assert_eq!(engine2.digest(), engine1.digest());
                    }
                }
            }
        };
    }

    #[cfg(feature = "bup")]
    test_engine!(bup, Bup);

    #[cfg(feature = "gear")]
    test_engine!(gear, Gear);
}
