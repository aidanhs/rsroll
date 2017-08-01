use super::Engine;
use rand::{Rng, SeedableRng, StdRng};

#[test]
fn bup_selftest()
{
    use super::Bup;
    const WINDOW_SIZE : usize = 1 << 6;

    const SELFTEST_SIZE: usize = 100000;
    let mut buf = [0u8; SELFTEST_SIZE];

    fn sum(buf : &[u8]) -> u32 {
        let mut e = Bup::new();
        e.roll(buf);
        e.digest()
    }

    let seed: &[_] = &[1, 2, 3, 4];
    let mut rng: StdRng = SeedableRng::from_seed(seed);
    for count in 0..SELFTEST_SIZE {
        buf[count] = rng.gen();
    }

    let sum1a: u32 = sum(&buf[0..]);
    let sum1b: u32 = sum(&buf[1..]);

    let sum2a: u32 = sum(&buf[SELFTEST_SIZE - WINDOW_SIZE*5/2 ..SELFTEST_SIZE - WINDOW_SIZE]);
    let sum2b: u32 = sum(&buf[0 .. SELFTEST_SIZE - WINDOW_SIZE]);

    let sum3a: u32 = sum(&buf[0 .. WINDOW_SIZE+4]);
    let sum3b: u32 = sum(&buf[3 .. WINDOW_SIZE+4]);

    assert_eq!(sum1a, sum1b);
    assert_eq!(sum2a, sum2b);
    assert_eq!(sum3a, sum3b);
}

pub fn test_data_1mb() -> Vec<u8> {
    let mut v = vec![0x0; 1024 * 1024];

    let seed: &[_] = &[2, 1, 255, 70];
    let mut rng: StdRng = SeedableRng::from_seed(seed);
    for i in 0..v.len() {
        v[i] = rng.gen();
    }

    v
}
