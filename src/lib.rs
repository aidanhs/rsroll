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

#[inline]
fn roll_windowed<E: Engine>(engine: &mut E, window_size: usize, data: &[u8]) {
    let last_window = data.windows(window_size).next_back().unwrap_or(data);
    for &b in last_window {
        engine.roll_byte(b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanorand::{Rng, WyRand};
    use std::collections::HashSet;

    pub(crate) fn rand_data(len: usize) -> Vec<u8> {
        let mut data = vec![0; len];
        let mut rng = WyRand::new_seed(0x01020304);
        rng.fill_bytes(&mut data);
        data
    }

    fn test_roll_byte_same_as_roll<E>()
    where
        E: Engine,
        E: Default,
        <E as Engine>::Digest: PartialEq,
        <E as Engine>::Digest: std::fmt::Debug,
    {
        let mut engine1 = E::default();
        let mut engine2 = E::default();

        let data = rand_data(1024);
        for (i, &b) in data.iter().enumerate() {
            engine1.roll_byte(b);

            engine2.reset();
            engine2.roll(&data[..=i]);
            assert_eq!(engine1.digest(), engine2.digest());

            let mut engine3 = E::default();
            engine3.roll(&data[..=i]);
            assert_eq!(engine1.digest(), engine3.digest());
        }
    }

    fn test_chunk_edge_correct_digest<E>()
    where
        E: Engine,
        E: Default,
        E::Digest: PartialEq,
        E::Digest: From<u16>,
        E::Digest: Copy,
        E::Digest: std::ops::BitAnd<Output = E::Digest>,
        E::Digest: std::fmt::Debug,
    {
        let mut engine1 = E::default();

        let data = rand_data(512 * 1024);
        let mut remaining = &data[..];
        let mask = E::Digest::from(0x0FFF);
        while let Some((i, digest)) =
            engine1.find_chunk_edge_cond(remaining, |e| e.digest() & mask == mask)
        {
            assert_eq!(digest & mask, mask);
            let mut engine2 = E::default();
            engine2.roll(&remaining[..i]);
            assert_eq!(engine2.digest(), digest);

            // Ensure no previous digests matched the mask
            engine2.reset();
            for &b in &remaining[..i - 1] {
                engine2.roll_byte(b);
                assert_ne!(engine2.digest() & mask, mask)
            }
            engine2.roll_byte(remaining[i - 1]);
            assert_eq!(engine2.digest() & mask, mask);
            assert_eq!(engine2.digest(), digest);

            remaining = &remaining[i..];
            engine2.reset();
            assert_eq!(engine2.digest(), engine1.digest());
        }
        let mut engine2 = E::default();
        engine2.roll(&data);
        assert_eq!(engine1.digest(), engine2.digest());
    }

    fn chunk<E, F>(mut data: &[u8], f: F) -> Vec<&[u8]>
    where
        E: Engine,
        E: Default,
        F: Fn(&E) -> bool,
    {
        let mut engine = E::default();
        let mut result = Vec::new();

        while let Some((i, _)) = engine.find_chunk_edge_cond(data, &f) {
            result.push(&data[..i]);
            data = &data[i..];
        }
        result.push(data);

        result
    }

    fn test_chunk_edge_converges<E>()
    where
        E: Engine,
        E: Default,
        E::Digest: PartialEq,
        E::Digest: From<u16>,
        E::Digest: Copy,
        E::Digest: std::ops::BitAnd<Output = E::Digest>,
        E::Digest: std::fmt::Debug,
    {
        let data = rand_data(64 * 1024);
        let mask = E::Digest::from(0x0FFF);

        let f = |e: &E| e.digest() & mask == mask;
        let chunks = chunk(&data, f);
        for i in 1..300 {
            let other_chunks = chunk(&data[i..], f);
            // ensure the last several chunks are equal
            let len = chunks.len() - 3;
            assert_eq!(
                chunks.windows(len).last().unwrap(),
                other_chunks.windows(len).last().unwrap()
            );
        }
    }

    fn test_chunk_edge_with_insert<E>()
    where
        E: Engine,
        E: Default,
        E::Digest: PartialEq,
        E::Digest: From<u16>,
        E::Digest: Copy,
        E::Digest: std::ops::BitAnd<Output = E::Digest>,
        E::Digest: std::fmt::Debug,
    {
        let mut data = rand_data(1024 * 1024);
        let mask = E::Digest::from(0x0FFF);
        let f = |e: &E| e.digest() & mask == mask;
        let chunks: HashSet<Vec<_>> = chunk(&data, f).iter().map(|x| x.to_vec()).collect();
        data.insert(5000, b'!');
        let other_chunks: HashSet<Vec<_>> = chunk(&data, f).iter().map(|x| x.to_vec()).collect();
        let different_chunks = chunks.symmetric_difference(&other_chunks).count();
        assert!(chunks.len() > 100);
        assert!(other_chunks.len() > 100);
        assert!(different_chunks < 4);
    }

    fn test_chunk_edge_incremental<E>()
    where
        E: Engine,
        E: Default,
        E::Digest: PartialEq,
        E::Digest: From<u16>,
        E::Digest: Copy,
        E::Digest: std::ops::BitAnd<Output = E::Digest>,
        E::Digest: std::fmt::Debug,
    {
        // Use a value that won't be a multiple of the window size (a prime)
        const INCREMENTAL_SIZE: usize = 307;
        let data = rand_data(1024 * 1024);
        let mask = E::Digest::from(0x0FFF);
        let f = |e: &E| e.digest() & mask == mask;

        let mut engine1 = E::default();
        let mut last_edge = 0;
        for (frame_i, frame) in data.chunks(INCREMENTAL_SIZE).enumerate() {
            let mut engine2 = E::default();
            let mut consumed = 0;
            while let Some((off, digest)) = engine1.find_chunk_edge_cond(&frame[consumed..], f) {
                consumed += off;
                let actual_edge = frame_i * INCREMENTAL_SIZE + consumed;
                assert_eq!(
                    engine2.find_chunk_edge_cond(&data[last_edge..], f),
                    Some((actual_edge - last_edge, digest)),
                );
                last_edge = actual_edge;
            }
            assert_eq!(
                engine2.find_chunk_edge_cond(
                    &data[last_edge..frame_i * INCREMENTAL_SIZE + frame.len()],
                    f
                ),
                None,
            );
        }
    }

    macro_rules! test_engine {
        ($name:ident, $engine:ty) => {
            mod $name {
                use super::*;

                #[test]
                fn roll_byte_same_as_roll() {
                    test_roll_byte_same_as_roll::<$engine>()
                }

                #[test]
                fn chunk_edge_correct_digest() {
                    test_chunk_edge_correct_digest::<$engine>()
                }

                #[test]
                fn chunk_edge_converges() {
                    test_chunk_edge_converges::<$engine>()
                }

                #[test]
                fn chunk_edge_with_insert() {
                    test_chunk_edge_with_insert::<$engine>()
                }

                #[test]
                fn chunk_edge_incremental() {
                    test_chunk_edge_incremental::<$engine>()
                }
            }
        };
    }

    #[cfg(feature = "bup")]
    test_engine!(bup, Bup);

    #[cfg(feature = "gear")]
    test_engine!(gear, Gear);
}
