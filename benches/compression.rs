use std::{fs::File, io::Read};

use criterion::{criterion_group, criterion_main, Criterion};
fn bench_compression(_c: &mut Criterion) {
    let mut test_file = File::open("test_data/sample_nodes.bin").expect("Failed to open test file");
    let before_size = test_file.metadata().unwrap().len() as usize;
    let quality = 11;
    let lg_window_size = 22;
    let mut input = brotli::CompressorReader::new(
        &mut test_file,
        4096, /* buffer size */
        quality as u32,
        lg_window_size as u32,
    );
    let mut after: Vec<u8> = Vec::new();
    input.read_to_end(&mut after).unwrap();
    let after_size = after.len();
    println!(
        "Original size: {}, Compressed size: {}",
        before_size, after_size
    );
    println!(
        "Compression ratio: {}",
        after_size as f64 / before_size as f64
    );
}

criterion_group!(benches, bench_compression);
criterion_main!(benches);
