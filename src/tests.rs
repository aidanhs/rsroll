mod rollsum {
    pub use super::super::*;
}

use rand::{Rng, SeedableRng, StdRng};

#[test]
fn bupsplit_selftest()
{
    use self::rollsum::rollsum_sum;
    use self::rollsum::WINDOW_SIZE;

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
