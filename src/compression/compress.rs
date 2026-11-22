#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{self, BufWriter, Read, Write};

    use brotli::CompressorWriter;

    // RUSTFLAGS="-Awarnings" cargo test --release test_brotli_compression -- --nocapture

    // 2.56x compression ratio
    #[test]
    fn test_brotli_compression() {
        let mut file = File::open("test_data/sample_nodes.bin").expect("Failed to open input file");
        let before_size = file.metadata().unwrap().len() as usize;
        let start = std::time::Instant::now();
        let std_out = Vec::new();
        let lgwin = 22;
        let mut buf = vec![0u8; 4096];
        let quality = 4;
        let mut writer = CompressorWriter::new(std_out, buf.len(), quality, lgwin);

        loop {
            match file.read(&mut buf[..]) {
                Err(e) => {
                    if let io::ErrorKind::Interrupted = e.kind() {
                        continue;
                    }
                    panic!("{}", e);
                }
                Ok(size) => {
                    if size == 0 {
                        match writer.flush() {
                            Err(e) => {
                                if let io::ErrorKind::Interrupted = e.kind() {
                                    continue;
                                }
                                panic!("{}", e)
                            }
                            Ok(_) => break,
                        }
                    }
                    match writer.write_all(&buf[..size]) {
                        Err(e) => panic!("{}", e),
                        Ok(_) => {}
                    }
                }
            }
        }
        let elapsed = start.elapsed();
        println!("Compression took: {:?}", elapsed);
        let res = writer.into_inner();
        let after_size = res.len();
        println!(
            "Original size: {}, Compressed size: {}",
            before_size, after_size
        );
        println!(
            "Compression ratio: {}",
            after_size as f64 / before_size as f64
        );
        let mut reader = brotli::Decompressor::new(res.as_slice(), 4096 /* buffer size */);
        let mut buf = [0u8; 4096];
        let mut writer = BufWriter::new(Vec::new());

        loop {
            match reader.read(&mut buf[..]) {
                Err(e) => {
                    if let io::ErrorKind::Interrupted = e.kind() {
                        continue;
                    }
                    panic!("{}", e);
                }
                Ok(size) => {
                    if size == 0 {
                        match writer.flush() {
                            Err(e) => {
                                if let io::ErrorKind::Interrupted = e.kind() {
                                    continue;
                                }
                                panic!("{}", e)
                            }
                            Ok(_) => break,
                        }
                    }
                    match writer.write_all(&buf[..size]) {
                        Err(e) => panic!("{}", e),
                        Ok(_) => {}
                    }
                }
            }
        }
        let reversed = writer.into_inner().unwrap();
        println!(
            "Decompressed size: {}, Original size: {}",
            reversed.len(),
            before_size
        );
        assert_eq!(
            reversed.len(),
            before_size,
            "Decompressed data does not match original"
        );
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_zstd_compression -- --nocapture
    #[test]
    fn test_zstd_compression() {
        use ruzstd::encoding::{compress, CompressionLevel};
        let mut file = File::open("test_data/sample_nodes.bin").expect("Failed to open input file");
        let before_size = file.metadata().unwrap().len() as usize;
        let start = std::time::Instant::now();
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf).expect("Failed to read file");
        let mut compressed_data = Vec::new();
        compress(
            buf.as_slice(),
            &mut compressed_data,
            CompressionLevel::Fastest,
        );
        let elapsed = start.elapsed();
        println!("Zstd compression took: {:?}", elapsed);
        let after_size = compressed_data.len();
        println!(
            "Original size: {}, Compressed size: {}",
            before_size, after_size
        );
        println!(
            "Compression ratio: {}",
            before_size as f64 / after_size as f64
        );
    }
}
