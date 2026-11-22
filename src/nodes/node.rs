use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, Write},
    time::{Duration, Instant},
};

use bincode::config;
use nix::libc::{self, pread};
use std::os::unix::io::AsRawFd;

use crate::{
    general::{key_path::KeyPath, set32::Set32},
    nodes::node_metadata::NodeType,
    util::get_set_file_deep,
};

pub fn get_node_dir(directory: &str, layout_config: &[usize], node_type: NodeType) -> String {
    let layout_str = match layout_config.is_empty() {
        true => "root".to_string(),
        false => layout_config
            .iter()
            .map(|&len| len.to_string())
            .collect::<Vec<String>>()
            .join("_"),
    };
    format!(
        "{}/L{}/{}/{}",
        directory,
        layout_config.len(),
        node_type,
        layout_str,
    )
}

pub fn get_node_path(directory: &str, layout_config: &[usize], node_type: NodeType) -> String {
    let layout_str = match layout_config.is_empty() {
        true => "root".to_string(),
        false => layout_config
            .iter()
            .map(|&len| len.to_string())
            .collect::<Vec<String>>()
            .join("_"),
    };
    format!(
        "{}/L{}/{}/{}/{}.bin",
        directory,
        layout_config.len(),
        node_type,
        layout_str,
        layout_str
    )
}

pub trait Node {
    fn get_key_path(&self) -> &KeyPath;
    const NODE_TYPE: NodeType;

    fn read_multiple_nodes2(
        directory: &str,
        layout_config: &[usize],
        positions: Vec<(usize, Set32, (usize, usize))>,
    ) -> Vec<(usize, Set32, Self)>
    where
        Self: Sized + bincode::Decode<()> + Send + Sync,
    {
        let fn_start = Instant::now();
        let path = get_node_path(directory, layout_config, Self::NODE_TYPE);
        // println!("Opened file: {}", &path);
        let start = Instant::now();
        let file = File::open(path).expect("File not found");
        println!("Opened file in {:?}", start.elapsed());
        let mut res = Vec::new();
        let mut total_seek_time = Duration::ZERO;
        let mut total_decode_time = Duration::ZERO;
        for (index_changed, target_combo_set, (start, num_bytes)) in positions {
            // println!("Reading node at start: {}, num_bytes: {}", start, num_bytes);
            let mut reader = BufReader::with_capacity(num_bytes, &file);
            let seek_time = Instant::now();
            reader.seek(std::io::SeekFrom::Start(start as u64)).unwrap();
            let seek_duration = seek_time.elapsed();
            total_seek_time += seek_duration;

            let decode_time = Instant::now();
            let nodes_inner: Vec<Self> = bincode::decode_from_std_read(
                &mut reader.take(num_bytes.try_into().unwrap()),
                config::standard(),
            )
            .expect("Failed to decode node");
            let decode_duration = decode_time.elapsed();
            total_decode_time += decode_duration;
            for node in nodes_inner {
                res.push((index_changed, target_combo_set, node));
            }
        }
        let fn_duration = fn_start.elapsed();
        println!("read_multiple_nodes2 took: {:?}", fn_duration);
        res
    }

    // #[cfg(target_os = "linux")]
    fn read_multiple_nodes2_nix(
        file: &File,
        positions: Vec<(usize, Set32, (usize, usize))>,
    ) -> Vec<(usize, Set32, Self)>
    where
        Self: Sized + bincode::Decode<()> + Send + Sync,
    {
        let fd = file.as_raw_fd();

        let mut res = Vec::new();

        for (index_changed, target_combo_set, (start, num_bytes)) in positions {
            let mut buffer = vec![0u8; num_bytes];
            let bytes_read = unsafe {
                pread(
                    fd,
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    num_bytes as libc::size_t,
                    start as libc::off_t,
                )
            };
            let mut slice = &buffer[..bytes_read as usize];
            let nodes_inner: Vec<Self> =
                bincode::decode_from_std_read(&mut slice, config::standard())
                    .expect("Failed to decode node");

            for node in nodes_inner {
                res.push((index_changed, target_combo_set, node));
            }
        }

        res
    }

    fn read_multiple_nodes2_nix_full_estimate(
        file: &File,
        positions: Vec<(usize, usize)>,
    ) -> Vec<(Set32, Vec<Self>)>
    where
        Self: Sized + bincode::Decode<()> + Send + Sync,
    {
        let fd = file.as_raw_fd();

        let mut res = Vec::new();

        for (start, num_bytes) in positions {
            let mut buffer = vec![0u8; num_bytes];
            let bytes_read = unsafe {
                pread(
                    fd,
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    num_bytes as libc::size_t,
                    start as libc::off_t,
                )
            };
            let mut slice = &buffer[..bytes_read as usize];
            let nodes_inner: Vec<Self> =
                bincode::decode_from_std_read(&mut slice, config::standard())
                    .expect("Failed to decode node");

            if nodes_inner.is_empty() {
                continue;
            }
            res.push((nodes_inner[0].get_key_path().key_set, nodes_inner));
        }

        res
    }

    fn read_multiple_nodes(
        directory: &str,
        layout_config: &[usize],
        positions: Vec<(usize, usize)>,
    ) -> Vec<Self>
    where
        Self: Sized + bincode::Decode<()> + Send + Sync,
    {
        let path = get_node_path(directory, layout_config, Self::NODE_TYPE);

        // println!("Opened file: {}", &path);

        let file = File::open(path).expect("File not found");
        let mut nodes = Vec::new();

        for (start, num_bytes) in positions {
            // println!("Reading node at start: {}, num_bytes: {}", start, num_bytes);
            let mut reader = BufReader::with_capacity(num_bytes, &file);
            reader.seek(std::io::SeekFrom::Start(start as u64)).unwrap();
            let nodes_inner: Vec<Self> = bincode::decode_from_std_read(
                &mut reader.take(num_bytes.try_into().unwrap()),
                config::standard(),
            )
            .expect("Failed to decode node");
            nodes.extend(nodes_inner);
        }
        nodes
    }
    fn write_nodes_to_disk<T: Node + bincode::Encode>(
        directory: &str,
        nodes: &Vec<T>,
        layout_config: &[usize],
    ) -> (usize, usize) {
        let path = get_node_path(directory, layout_config, T::NODE_TYPE);
        // Create directory structure if it doesn't exist
        let file = get_set_file_deep(
            &path,
            &std::fs::OpenOptions::new().append(true).create(true),
        )
        .unwrap();

        let start_pos = file.metadata().unwrap().len() as usize;
        let mut writer = BufWriter::new(file);
        // Get the current position before writing

        let written_bytes = bincode::encode_into_std_write(nodes, &mut writer, config::standard())
            .expect("Failed to encode nodes");

        (start_pos, written_bytes)
    }
    fn write_nodes_to_disk_pre_encoded(nodes: &Vec<u8>, file: &mut File) -> usize {
        // Create directory structure if it doesn't exist
        let start_pos = file.metadata().unwrap().len() as usize;
        file.write_all(nodes).expect("Failed to write nodes");
        start_pos
    }
    fn update_nodes_to_disk_pre_encoded(nodes: &Vec<u8>, file: &mut File, start: usize) {
        let mut writer = BufWriter::new(file);
        writer
            .seek(std::io::SeekFrom::Start(start as u64))
            .expect("Failed to seek to position");
        writer.write_all(nodes).expect("Failed to write nodes");
    }
    fn update_nodes<T: Node + bincode::Encode>(
        directory: &str,
        nodes: &Vec<T>,
        layout_config: &[usize],
        start: usize,
    ) -> usize {
        let path = get_node_path(directory, layout_config, T::NODE_TYPE);
        // Create directory structure if it doesn't exist
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("Failed to open file");
        let before_len = file.metadata().unwrap().len() as usize;

        let mut writer = BufWriter::new(file);
        // Seek to the target writing spot
        writer
            .seek(std::io::SeekFrom::Start(start as u64))
            .expect("Failed to seek to position");
        let written_bytes = bincode::encode_into_std_write(nodes, &mut writer, config::standard())
            .expect("Failed to encode nodes");
        writer.flush().expect("Failed to flush writer");

        let file = File::open(&path).expect("Failed to open file after writing");
        let after_len = file.metadata().unwrap().len() as usize;
        debug_assert_eq!(after_len, before_len, "Updating incorrectly");
        written_bytes
    }
}
