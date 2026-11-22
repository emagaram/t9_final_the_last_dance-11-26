use criterion::{criterion_group, criterion_main, Criterion};
use micromap::Map;
use rand::seq::IndexedRandom;

fn linear_search(vec: &[i32], target: i32) -> Option<usize> {
    vec.iter().position(|&x| x == target)
}

fn bench_searches(c: &mut Criterion) {
    let mut rng = rand::rng();
    let data: Vec<i32> = (0..20).collect();
    let targets: Vec<i32> = data.sample(&mut rng, 10).cloned().collect();
    let mut micromap: Map<i32, usize, 27> = Map::new();
    for (i, &value) in data.iter().enumerate() {
        micromap.insert(value, i);
    }
    c.bench_function("linear_search", |b| {
        b.iter(|| {
            for &target in &targets {
                let _ = linear_search(&data, target);
            }
        })
    });

    c.bench_function("micromap_search", |b| {
        b.iter(|| {
            for &target in &targets {
                let _ = micromap.get(&target);
            }
        })
    });
}

criterion_group!(benches, bench_searches);
criterion_main!(benches);
