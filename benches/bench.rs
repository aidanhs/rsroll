use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nanorand::Rng;
use rollsum::Engine;

fn bench_roll_byte(c: &mut Criterion) {
    const SIZE: usize = 128 * 1024;

    let mut data = vec![0u8; SIZE];
    let mut rng = nanorand::WyRand::new_seed(0x01020304);
    rng.fill_bytes(&mut data);

    let mut group = c.benchmark_group("roll");
    group.throughput(Throughput::Bytes(SIZE as u64));

    macro_rules! bench_engine {
        ($name:ident) => {{
            group.bench_function(concat!(stringify!($name), "/byte_by_byte"), |b| {
                let mut engine = rollsum::$name::new();
                b.iter(|| {
                    for _ in 0..SIZE {
                        engine.roll_byte(black_box(0));
                    }
                });
            });

            group.bench_function(concat!(stringify!($name), "/all"), |b| {
                let mut engine = rollsum::$name::new();
                b.iter(|| {
                    engine.roll(black_box(&data));
                    black_box(engine.digest());
                });
            });

            group.bench_function(concat!(stringify!($name), "/split"), |b| {
                let mut engine = rollsum::$name::new();
                b.iter(|| {
                    let mut remaining = black_box(&data[..]);
                    while let Some((new_i, digest)) = engine.find_chunk_edge(remaining) {
                        black_box((new_i, digest));
                        remaining = &remaining[new_i..];
                    }
                });
            });
        }};
    }

    #[cfg(feature = "gear")]
    bench_engine!(Gear);
    #[cfg(feature = "bup")]
    bench_engine!(Bup);
}

criterion_group!(benches, bench_roll_byte);
criterion_main!(benches);
