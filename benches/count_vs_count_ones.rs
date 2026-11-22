use criterion::{criterion_group, criterion_main, Criterion};
// Import Set32 and ALPHABET_SET32 from your crate or module
use t9_project::{general::set32::Set32, util::ALPHABET_SET32};
#[cfg(test)]
fn bench_count_vs_count_ones(c: &mut Criterion) {
    // Prepare a variety of Set32 values

    let sets = [
        Set32::EMPTY,
        Set32::fill_from_right(1),
        Set32::fill_from_right(10),
        Set32::fill_from_right(32),
        Set32::fill_from_left(5),
        Set32::fill_from_left(32),
        ALPHABET_SET32,
        Set32::EMPTY.add(3).add(7).add(15).add(31),
    ];

    c.bench_function("set32_count_vs_count_ones", |b| {
        b.iter(|| {
            for set in &sets {
                let iter_count = set.into_iter().count();
                let ones_count = set.count_ones();
                assert_eq!(
                    iter_count, ones_count as usize,
                    "Mismatch for set: {:?}",
                    set
                );
            }
        })
    });

    let set = Set32::fill_from_right(24).add(31).add(29).add(17);
    let iterations = 100_000;

    c.bench_function("set32_count_iter", |b| {
        b.iter(|| {
            let mut sum = 0;
            for _ in 0..iterations {
                sum += set.count();
            }
            sum
        })
    });

    c.bench_function("set32_count_ones", |b| {
        b.iter(|| {
            let mut sum = 0;
            for _ in 0..iterations {
                sum += set.count_ones();
            }
            sum
        })
    });
}

criterion_group!(benches, bench_count_vs_count_ones);
criterion_main!(benches);
