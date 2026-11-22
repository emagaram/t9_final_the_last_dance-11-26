use core::panic;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
};

use crate::{
    evaluation::final_evaluate::criteria_dict::CriteriaDict,
    evaluation::final_evaluate::scores_hm_tree_node::{
        build_dyn_trie, build_static_trie, DynNode, NodeInnerNew32, StaticTrie32, StaticTrie64,
        StaticTrieOld64,
    },
    evaluation::final_evaluate::union_find,
    general::set32::Set32,
    globals::NUM_LETTERS,
};
use bincode::config;
use dashmap::{DashMap, Entry};
use gxhash::{HashMap, HashMapExt};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

fn filter_sort_letter_seq(groups: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    groups
        .into_iter()
        .filter(|k| k.len() > 1)
        .sorted_by(|a, b| {
            // First compare by length (descending)
            b.len()
                .cmp(&a.len())
                // Then compare by values (descending
                .then_with(|| b.iter().cmp(a.iter()))
        })
        .map(|group| group.into_iter().map(|num| num as u8).collect())
        .collect()
}

fn letter_seqs_to_keys(keys: &Vec<Vec<u8>>) -> Vec<Set32> {
    let mut sets = Vec::with_capacity(keys.len());
    for key in keys {
        let mut set = Set32::EMPTY;
        for &item in key {
            set = set.add(item as u32);
        }
        sets.push(set)
    }
    sets
}

fn create_set32_key(keys: &Vec<Vec<u8>>) -> Set32 {
    let mut set = Set32::EMPTY;
    for key in keys {
        if key.len() <= 1 {
            panic!(
                "Key group must have at least 2 elements, got: {}",
                key.len()
            );
        }
        for &item in key {
            set = set.add(item as u32);
        }
    }
    set
}

fn process_length_group_vec_par(
    len: &u8,
    words: &[(Vec<u8>, f64)],
    dash_map: &DashMap<Set32, Vec<(Vec<Set32>, f64)>>,
) {
    println!(
        "Len {} processing pairs in parallel: {}",
        len,
        words.len() * words.len() / 2
    );

    // Create pairs of indices to process
    let pairs: Vec<(usize, usize)> = (0..words.len())
        .flat_map(|i| (i + 1..words.len()).map(move |j| (i, j)))
        .collect();

    // Process each word pair in parallel
    pairs.into_par_iter().for_each(|(i, j)| {
        let (word_i, freq_i) = &words[i];
        let (word_j, freq_j) = &words[j];

        if freq_i < freq_j {
            panic!("Score_i is less than score_j: {} < {}", freq_i, freq_j);
        }

        // ann, thh => [(a,t),(n,h),(n,h)] but numerically
        let pairs = CriteriaDict::criteria_get(word_i, word_j);
        // [[a,t],[n,h]]
        let keys: Vec<Vec<u8>> = filter_sort_letter_seq(union_find::connected_groups(&pairs));
        // atnh
        let key = create_set32_key(&keys);
        // [at,nh]
        let keys = letter_seqs_to_keys(&keys);

        // Use thread-safe insertion
        match dash_map.entry(key) {
            Entry::Occupied(mut e) => e.get_mut().push((keys, *freq_j as f64)),
            Entry::Vacant(e) => {
                e.insert(vec![(keys, *freq_j as f64)]);
            }
        }
    });
}

fn process_length_group_vec(
    len: &u8,
    words: &[(Vec<u8>, f64)],
    dash_map: &DashMap<Set32, Vec<(Vec<Set32>, f64)>>,
) {
    println!(
        "Len {} processing pairs: {}",
        len,
        words.len() * words.len() / 2
    );
    // Process each word pair sequentially
    for i in 0..words.len() {
        let (word_i, freq_i) = &words[i];
        for j in i + 1..words.len() {
            let (word_j, freq_j) = &words[j];

            if freq_i < freq_j {
                panic!("Score_i is less than score_j: {} < {}", freq_i, freq_j);
            }

            let pairs = CriteriaDict::criteria_get(&word_i, &word_j);
            let keys: Vec<Vec<u8>> = filter_sort_letter_seq(union_find::connected_groups(&pairs));
            let key = create_set32_key(&keys);
            let keys = letter_seqs_to_keys(&keys);
            match dash_map.entry(key) {
                Entry::Occupied(mut e) => e.get_mut().push((keys, *freq_j as f64)),
                Entry::Vacant(e) => {
                    e.insert(vec![(keys, *freq_j as f64)]);
                }
            }
        }
    }
}

pub fn gen_hm_vec2(dict: &str) -> HashMap<Set32, Vec<(Vec<Set32>, f64)>> {
    use crate::general::freq_list;
    use std::sync::Arc;

    let mut freq_dict_len: HashMap<u8, Vec<(Vec<u8>, f64)>> = HashMap::new();
    let freq_list = freq_list::create_freq_list64(dict);

    // Group by length
    for (word, score) in freq_list.into_iter() {
        let len = word.len();
        freq_dict_len
            .entry(len as u8)
            .and_modify(|words| words.push((word.clone(), score)))
            .or_insert_with(|| vec![(word, score)]);
    }

    let result: DashMap<Set32, Vec<(Vec<Set32>, f64)>> = DashMap::new();
    let total_groups = freq_dict_len.len();
    let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    freq_dict_len.iter().for_each(|(len, words)| {
        // Process this length group
        process_length_group_vec_par(len, words, &result);

        // Update and print progress
        let completed_count = completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        println!(
            "HmVec progress: {}/{} length groups completed ({:.1}%)",
            completed_count,
            total_groups,
            (completed_count as f32 / total_groups as f32) * 100.0
        );
    });
    println!("Converting DashMap to HashMap");
    let result = result.into_iter().map(|(k, v)| (k, v)).collect();
    println!("Conversion complete\n");
    result
}

/// For every pair of same-length words in `dict`, computes the letter groups that
/// the pair forces to share a key (via `CriteriaDict` + union-find), then buckets
/// the results by the union of those letters (`Set32`). Each bucket holds
/// `(letter groups, score of the lower-frequency word)` entries, which
/// `gen_hm_dyn_tree` later turns into one trie per bucket.
///
/// Different length groups are processed in parallel; pairs within a group are
/// processed sequentially (see `gen_hm_vec2` for the fully parallel variant).
pub fn gen_hm_vec(dict: &str) -> HashMap<Set32, Vec<(Vec<Set32>, f64)>> {
    use crate::general::freq_list;
    use rayon::prelude::*;
    use std::sync::Arc;

    let mut freq_dict_len: HashMap<u8, Vec<(Vec<u8>, f64)>> = HashMap::new();
    let freq_list = freq_list::create_freq_list64(dict);

    // Group words by length; only same-length words are compared
    for (word, score) in freq_list.into_iter() {
        let len = word.len();
        freq_dict_len
            .entry(len as u8)
            .and_modify(|words| words.push((word.clone(), score)))
            .or_insert_with(|| vec![(word, score)]);
    }

    let result: DashMap<Set32, Vec<(Vec<Set32>, f64)>> = DashMap::new();
    let total_groups = freq_dict_len.len();
    let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    freq_dict_len.par_iter().for_each(|(len, words)| {
        // Compare all pairs in this length group, inserting into the shared DashMap
        process_length_group_vec(len, words, &result);

        // Update and print progress
        let completed_count = completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        println!(
            "HmVec progress: {}/{} length groups completed ({:.1}%)",
            completed_count,
            total_groups,
            (completed_count as f32 / total_groups as f32) * 100.0
        );
    });
    println!("Converting DashMap to HashMap");
    let result = result.into_iter().map(|(k, v)| (k, v)).collect();
    println!("Conversion complete\n");
    result
}

pub fn gen_hm_dyn_tree(
    hm_pairs: HashMap<Set32, Vec<(Vec<Set32>, f64)>>, // comes from gen_hm_vec or gen_hm_vec2
) -> HashMap<Set32, DynNode> {
    let result: DashMap<Set32, DynNode> = DashMap::with_capacity(hm_pairs.len());
    println!("Building dynamic trie from pairs");
    hm_pairs.into_par_iter().for_each(|(key, pairs)| {
        result.insert(key, build_dyn_trie(pairs));
    });
    println!("Dynamic trie construction complete.\nConverting to HashMap");
    let result = result.into_iter().map(|(k, v)| (k, v)).collect();
    println!("Conversion complete\n");
    result
}
use rayon::prelude::*;

pub fn gen_hm_dyn_tree_chunked(
    hm_pairs: HashMap<Set32, Vec<(Vec<Set32>, f64)>>,
) -> HashMap<Set32, DynNode> {
    let result: DashMap<Set32, DynNode> = DashMap::new();
    println!("Building dynamic trie from pairs");

    // Convert to Vec and process in chunks
    let pairs_vec: Vec<_> = hm_pairs.into_iter().collect();
    let chunk_size = 1000; // Adjust based on your memory constraints

    for chunk in pairs_vec.chunks(chunk_size) {
        chunk.par_iter().for_each(|(key, pairs)| {
            result.insert(key.clone(), build_dyn_trie(pairs.clone()));
        });
        // Each chunk's memory is freed after processing
    }

    println!("Dynamic trie construction complete.\nConverting to HashMap");
    let result = result.into_iter().collect();
    println!("Conversion complete\n");
    result
}

pub fn gen_hm_static_tree64(hm_dyn_tree: HashMap<Set32, DynNode>) -> HashMap<Set32, StaticTrie64> {
    let result: DashMap<Set32, StaticTrie64> = DashMap::new();
    println!("Building static trie from dynamic trie");
    hm_dyn_tree.into_par_iter().for_each(|(key, root)| {
        result.insert(key, build_static_trie(root));
    });
    println!("Static trie construction complete.\nConverting to HashMap");
    let result = result.into_iter().collect();
    println!("Conversion complete\n");
    result
}

pub fn gen_hm_static_tree32(hm_dyn_tree: HashMap<Set32, DynNode>) -> HashMap<Set32, StaticTrie32> {
    let hm = gen_hm_static_tree64(hm_dyn_tree);
    hm.into_iter()
        .map(|(key, trie)| {
            let nodes: Vec<_> = trie
                .nodes
                .into_iter()
                .map(|node| NodeInnerNew32 {
                    score: node.score as f32,
                    first_edge: node.first_edge,
                    edge_count: node.edge_count,
                })
                .collect();
            (
                key,
                StaticTrie32 {
                    nodes,
                    edges: trie.edges,
                },
            )
        })
        .collect()
}

pub fn save(hm_static_tree: &HashMap<Set32, StaticTrie64>, path: &str) {
    match File::create(path) {
        Ok(mut file) => {
            let encoded: Vec<u8> =
                bincode::encode_to_vec(&hm_static_tree, config::standard()).unwrap();
            file.write_all(&encoded).unwrap();
        }
        _ => {
            eprintln!("Failed to create hm_tree file");
        }
    }
}

pub fn save_stream64(hm_static_tree: &HashMap<Set32, StaticTrie64>, path: &str) {
    match File::create(path) {
        Ok(file) => {
            let mut writer = BufWriter::new(file);
            bincode::encode_into_std_write(hm_static_tree, &mut writer, config::standard())
                .unwrap();
        }
        Err(_) => {
            eprintln!("Failed to create hm_tree file");
        }
    }
}
pub fn save_stream32(hm_static_tree: &HashMap<Set32, StaticTrie32>, path: &str) {
    match File::create(path) {
        Ok(file) => {
            let mut writer = BufWriter::new(file);
            bincode::encode_into_std_write(hm_static_tree, &mut writer, config::standard())
                .unwrap();
        }
        Err(_) => {
            eprintln!("Failed to create hm_tree file");
        }
    }
}

pub fn read(path: &str) -> Result<HashMap<Set32, StaticTrie64>, std::io::Error> {
    let file_data = fs::read(path)?;
    let (result, _) = bincode::decode_from_slice::<HashMap<Set32, StaticTrie64>, _>(
        &file_data,
        config::standard(),
    )
    .expect("Failed to decode hm_tree data");

    Ok(result)
}

use std::io::BufReader;

pub fn read_stream_old(path: &str) -> Result<HashMap<Set32, StaticTrieOld64>, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let result = bincode::decode_from_std_read(&mut reader, config::standard())
        .expect("Failed to decode hm_tree data");
    Ok(result)
}

pub fn read_stream(path: &str) -> Result<HashMap<Set32, StaticTrie64>, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let result = bincode::decode_from_std_read(&mut reader, config::standard())
        .expect("Failed to decode hm_tree data");
    Ok(result)
}

pub fn read_stream32(path: &str) -> Result<HashMap<Set32, StaticTrie32>, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let result = bincode::decode_from_std_read(&mut reader, config::standard())
        .expect("Failed to decode hm_tree data");
    Ok(result)
}

pub fn create_letter_idx_map(
    letter_idx_map: &[Option<usize>; NUM_LETTERS],
    key: Set32,
    count: usize,
) -> [Option<usize>; NUM_LETTERS] {
    let mut res = letter_idx_map.clone();
    for i in key.ones_indices() {
        res[i as usize] = Some(count);
    }
    res
}

pub fn get_intersection_score32(
    path: &[Set32],
    path_key: Set32,
    letter_idx_map: &[Option<usize>; NUM_LETTERS],
    hm_static_tree: &HashMap<Set32, StaticTrie32>,
) -> f32 {
    fn dfs(
        tree: &StaticTrie32,
        letter_idx_map: &[Option<usize>; NUM_LETTERS],
        node_idx: usize,
        path: &[Set32],
    ) -> f64 {
        let mut score: f64 = 0.0;
        if path.is_empty() {
            panic!("Path cannot be empty");
        }
        let (children, start) = tree.get_children(node_idx);
        for (i, child) in children.iter().enumerate() {
            let match_idx = letter_idx_map[child.0.trailing_zeros() as usize];
            if match_idx.is_none() {
                continue; // Skip if no match index
            }
            let compare_key = path[match_idx.unwrap()];
            if compare_key.intersect(*child) == *child {
                score += tree.get_node(i + start as usize).score as f64;
                score += dfs(tree, letter_idx_map, i + start as usize, path);
            }
        }
        score
    }
    #[cfg(debug_assertions)]
    {
        if path.is_empty() {
            panic!("Path cannot be empty");
        }
    }

    let tree = hm_static_tree.get(&path_key);
    match tree {
        Some(static_trie) => dfs(static_trie, &letter_idx_map, 0, path) as f32,
        None => return 0.0, // Path key not found in the trie
    }
}

// RUSTFLAGS="-Awarnings" cargo test --release scores_hm_tree
#[cfg(test)]
mod tests {
    use crate::{create_set32_str, create_set32s_vec};
    use std::time::Instant;

    use super::*;

    /*

    at de hn: 1.4
    ath end: 1.2
    hande: 1.2
    it os: 1.0
    tof: 0.9
    io sf: 0.9
    ab: 0.9
    ad: 1.5
    {

        "thed": 1.5,
        "hane": 1.2,
        "andd": 1.4,
        "and": 2.2,
        "ana": 1.5,
        "to": 2.0,
        "is": 1.0,
        "of": 0.9,
        "a": 1.0,
        "b": 0.9
    }
     */

    // RUSTFLAGS="-Awarnings" cargo test --release test_read_vs_read_stream_performance -- --nocapture
    #[test]
    fn test_read_vs_read_stream_performance() {
        let dict = "word_freq_2000.json";

        // Generate and save the data structure
        let hm_vec = gen_hm_vec(dict);
        let hm_dyn_tree = gen_hm_dyn_tree(hm_vec);
        let hm_static_tree = gen_hm_static_tree64(hm_dyn_tree);

        let path = "test_hm_static_tree_read.bin";
        save(&hm_static_tree, path);

        // Test regular read
        let start = Instant::now();
        let loaded_regular = read(path).expect("Failed to read using regular method");
        let duration_read = start.elapsed();

        // Test stream read
        let start = Instant::now();
        let loaded_stream = read_stream(path).expect("Failed to read using stream method");
        let duration_stream = start.elapsed();

        println!("Regular read took: {:?}", duration_read);
        println!("Buffered stream read took: {:?}", duration_stream);

        // Verify both methods loaded the same data
        assert_eq!(hm_static_tree.len(), loaded_regular.len());
        assert_eq!(hm_static_tree.len(), loaded_stream.len());

        // Verify the content is the same
        for (key, static_trie) in &hm_static_tree {
            let loaded_trie_regular = loaded_regular
                .get(key)
                .expect("Key not found in regular loaded tree");
            let loaded_trie_stream = loaded_stream
                .get(key)
                .expect("Key not found in stream loaded tree");

            assert_eq!(static_trie.nodes.len(), loaded_trie_regular.nodes.len());
            assert_eq!(static_trie.nodes.len(), loaded_trie_stream.nodes.len());
            assert_eq!(static_trie.edges.len(), loaded_trie_regular.edges.len());
            assert_eq!(static_trie.edges.len(), loaded_trie_stream.edges.len());
        }

        // Clean up
        std::fs::remove_file(path).unwrap();

        println!(
            "Speed improvement with buffered reads: {:.2}x",
            duration_read.as_secs_f64() / duration_stream.as_secs_f64()
        );
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_save_vs_save_stream_performance -- --nocapture
    #[test]
    fn test_save_vs_save_stream_performance() {
        let dict = "word_freq_20000.json";

        // Generate the data structure to save
        let hm_vec = gen_hm_vec(dict);
        let hm_dyn_tree = gen_hm_dyn_tree(hm_vec);
        let hm_static_tree = gen_hm_static_tree64(hm_dyn_tree);

        // Test save
        let save_path = "hm_static_tree_regular.bin";
        let start = Instant::now();
        save(&hm_static_tree, save_path);
        let duration_save = start.elapsed();

        // Test save_stream
        let stream_path = "hm_static_tree_stream.bin";
        let start = Instant::now();
        save_stream64(&hm_static_tree, stream_path);
        let duration_stream = start.elapsed();

        // Check file sizes
        let save_size = std::fs::metadata(save_path).unwrap().len();
        let stream_size = std::fs::metadata(stream_path).unwrap().len();

        println!("Regular save took: {:?}", duration_save);
        println!("Buffered stream save took: {:?}", duration_stream);
        println!("Regular save file size: {} bytes", save_size);
        println!("Buffered stream file size: {} bytes", stream_size);

        // Verify both files contain valid data by loading them
        let loaded_regular = read(save_path).expect("Failed to read regular saved file");
        let loaded_stream = read(stream_path).expect("Failed to read stream saved file");

        assert_eq!(hm_static_tree.len(), loaded_regular.len());
        assert_eq!(hm_static_tree.len(), loaded_stream.len());
        assert_eq!(save_size, stream_size, "File sizes should be identical");

        // Clean up
        std::fs::remove_file(save_path).unwrap();
        std::fs::remove_file(stream_path).unwrap();

        println!(
            "Speed improvement with buffered writes: {:.2}x",
            duration_save.as_secs_f64() / duration_stream.as_secs_f64()
        );
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_read_tree_max_edge_count_and_node_start -- --nocapture
    #[test]
    fn test_read_tree_max_edge_count_and_node_start() {
        // let dict = "word_freq_2000.json"; // Use a reasonable sized dictionary
        // Generate and save the data structure
        // let hm_vec = gen_hm_vec(dict);
        // let hm_dyn_tree = gen_hm_dyn_tree(hm_vec);

        // let path = "test_max_edge_count.bin";
        // save(&hm_static_tree, path);

        // Read the tree back
        println!("Reading the static tree");
        let loaded_tree = read_stream_old("scores").expect("Failed to read the static tree");
        println!("Static tree loaded");
        // Count max edge_count and node start values
        let mut max_edge_count = 0;
        let mut max_node_first_edge = 0;
        let mut node_count: usize = 0;
        let mut edges_count: usize = 0;
        for (_, trie) in &loaded_tree {
            for node in &trie.nodes {
                node_count += 1;
                if node.edge_count > max_edge_count {
                    max_edge_count = node.edge_count;
                }
                if node.first_edge > max_node_first_edge {
                    max_node_first_edge = node.first_edge;
                }
            }
            edges_count += trie.edges.len();
        }

        println!("Num nodes in the tree: {}", node_count);
        println!("Num edges in the tree: {}", edges_count);
        println!("Maximum edge count in the tree: {}", max_edge_count);
        println!(
            "Maximum node first edge in the tree: {}",
            max_node_first_edge
        );

        // Make some basic assertions to ensure values are reasonable
        assert!(max_edge_count > 0, "Edge count should be positive");
        assert!(max_node_first_edge > 0, "Node start should be positive");
        // assert!(max_node_first_edge < 1000000, "Node start should be within reasonable bounds");
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_hm_vec_vs_hm_vec2_performance -- --nocapture
    #[test]
    fn test_hm_vec_vs_hm_vec2_performance() {
        let dict = "word_freq_2000.json";

        // Test gen_hm_vec
        let start = Instant::now();
        let hm_vec = gen_hm_vec(dict);
        let duration_vec = start.elapsed();

        // Test gen_hm_vec2
        let start = Instant::now();
        let hm_vec2 = gen_hm_vec2(dict);
        let duration_vec2 = start.elapsed();

        println!("gen_hm_vec took: {:?}", duration_vec);
        println!("gen_hm_vec2 took: {:?}", duration_vec2);

        // Verify results are equivalent
        assert_eq!(
            hm_vec.len(),
            hm_vec2.len(),
            "Both functions should generate same number of keys"
        );

        for (key, pairs) in &hm_vec {
            let pairs2 = hm_vec2.get(key).expect("Key should exist in both hashmaps");
            assert_eq!(
                pairs.len(),
                pairs2.len(),
                "Number of pairs for key {:?} should match",
                key
            );

            // Sort both to ensure consistent comparison
            let mut pairs_sorted = pairs.clone();
            let mut pairs2_sorted = pairs2.clone();

            pairs_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            pairs2_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            for i in 0..pairs_sorted.len() {
                assert!(
                    (pairs_sorted[i].1 - pairs2_sorted[i].1).abs() < 0.00001,
                    "Scores should match for pair at index {}",
                    i
                );
            }
        }

        // Optional: Check if parallel version is faster (though it might not be for small dictionaries)
        println!(
            "Speed improvement: {:.2}x",
            duration_vec.as_secs_f64() / duration_vec2.as_secs_f64()
        );
    }
    // RUSTFLAGS="-Awarnings" cargo test --release test_hm_tree_read_save -- --nocapture
    #[test]
    fn test_hm_tree_read_save() {
        let dict = "word_freq_2000.json"; // Replace with your actual dictionary file
        let hm_vec = gen_hm_vec(dict);
        let hm_dyn_tree = gen_hm_dyn_tree(hm_vec);
        let hm_static_tree = gen_hm_static_tree64(hm_dyn_tree);

        println!("Saving tree");
        let path = "test_hm_static_tree.bin";
        save(&hm_static_tree, path);
        println!("Tree saved");
        println!("Loading tree");
        let loaded_tree = read(path).expect("Failed to read the static tree");
        println!("Tree loaded");
        assert_eq!(hm_static_tree.len(), loaded_tree.len());
        for (key, static_trie) in &hm_static_tree {
            let loaded_trie = loaded_tree.get(key).expect("Key not found in loaded tree");
            assert_eq!(static_trie.nodes.len(), loaded_trie.nodes.len());
            assert_eq!(static_trie.edges.len(), loaded_trie.edges.len());
        }
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_hm_tree_get_intersection_score -- --nocapture
    #[test]
    fn test_hm_tree_get_intersection_score() {
        let dict = "word_freq_med.json"; // Replace with your actual dictionary file
        let hm_vec = gen_hm_vec(dict);
        let hm_dyn_tree = gen_hm_dyn_tree(hm_vec);
        let hm_static_tree = gen_hm_static_tree32(hm_dyn_tree);

        let path = vec![create_set32_str!("athend")];

        let path_key = create_set32_str!("athend");
        let letter_idx_map = create_letter_idx_map(&[None; NUM_LETTERS], path_key, 0);
        let score = get_intersection_score32(&path, path_key, &letter_idx_map, &hm_static_tree);

        assert!((score - 2.6).abs() < 0.00001);

        let path = vec![
            create_set32_str!("at"),
            create_set32_str!("de"),
            create_set32_str!("hn"),
        ];
        let path_key = create_set32_str!("athend");
        let mut letter_idx_map = [None; NUM_LETTERS];
        for (i, key) in path.iter().enumerate() {
            letter_idx_map = create_letter_idx_map(&letter_idx_map, *key, i);
        }
        let score = get_intersection_score32(&path, path_key, &letter_idx_map, &hm_static_tree);
        println!("Score: {}", score);
        assert!((score - 1.4).abs() < 0.00001);
    }

    #[test]
    fn test_hm_tree_gen_hm_vec() {
        let dict = "word_freq_test.json";
        let hm_vec = gen_hm_vec(dict);
        assert!(
            !hm_vec.is_empty(),
            "Generated HM vector should not be empty"
        );
        let dict = "word_freq_med.json";
        let hm_vec = gen_hm_vec(dict);
        assert_eq!(hm_vec.len(), 7);
        let sets = create_set32s_vec!("andeht,adehn,itos,tof,iosf,ab,ad");

        for (key, pairs) in &hm_vec {
            assert!(
                sets.contains(&key),
                "Key {:?} not found in expected set",
                key
            );
            assert!(
                !pairs.is_empty(),
                "Pairs for key {:?} should not be empty",
                key
            );
        }
        assert_eq!(hm_vec.get(&create_set32_str!("athend")).unwrap().len(), 2);
    }

    #[test]
    fn test_gen_hm_dyn_tree() {
        let dict = "word_freq_med.json"; // Replace with your actual dictionary file
        let hm_vec = gen_hm_vec(dict);
        let hm_dyn_tree = gen_hm_dyn_tree(hm_vec);
        assert!(
            !hm_dyn_tree.is_empty(),
            "Generated HM dynamic tree should not be empty"
        );
        assert_eq!(hm_dyn_tree.len(), 7);
        let sets = create_set32s_vec!("andeht,adehn,itos,tof,iosf,ab,ad");
        for (key, node) in &hm_dyn_tree {
            assert!(
                sets.contains(&key),
                "Key {:?} not found in expected set",
                key
            );
            assert!(
                node.children.len() > 0,
                "Pairs for key {:?} should not be empty",
                key
            );
        }
        assert_eq!(
            hm_dyn_tree
                .get(&create_set32_str!("athend"))
                .unwrap()
                .children
                .len(),
            2
        );
        let p1 = hm_dyn_tree
            .get(&create_set32_str!("athend"))
            .unwrap()
            .find_in_children(create_set32_str!("at"))
            .unwrap()
            .find_in_children(create_set32_str!("de"))
            .unwrap()
            .find_in_children(create_set32_str!("hn"))
            .unwrap();
        assert_eq!(p1.score, 1.4);
        let p2 = hm_dyn_tree
            .get(&create_set32_str!("athend"))
            .unwrap()
            .find_in_children(create_set32_str!("ath"))
            .unwrap()
            .find_in_children(create_set32_str!("end"))
            .unwrap();
        assert_eq!(p2.score, 1.2);
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_gen_hm_static_tree -- --nocapture
    #[test]
    fn test_gen_hm_static_tree() {
        let dict = "word_freq_med.json"; // Replace with your actual dictionary file
        let hm_vec = gen_hm_vec(dict);
        let hm_dyn_tree = gen_hm_dyn_tree(hm_vec.clone());
        let hm_static_tree = gen_hm_static_tree64(hm_dyn_tree);
        let hm_dyn_tree = gen_hm_dyn_tree(hm_vec);
        assert!(
            !hm_static_tree.is_empty(),
            "Generated HM static tree should not be empty"
        );
        assert_eq!(hm_static_tree.len(), 7);
        let sets = create_set32s_vec!("andeht,adehn,itos,tof,iosf,ab,ad");
        for (key, _) in &hm_static_tree {
            assert!(
                sets.contains(&key),
                "Key {:?} not found in expected set",
                key
            );
        }

        // Check if static tree and dyn tree are equivalent
        for (key, static_trie) in &hm_static_tree {
            let dyn_node = hm_dyn_tree.get(key).expect("Key should exist in dyn_tree");
            // Create a way to traverse the static trie
            let mut dyn_paths = Vec::new();
            fn collect_paths_inner(
                node: &DynNode,
                path: Vec<Set32>,
                score: f64,
                paths: &mut Vec<(Vec<Set32>, f64)>,
            ) {
                if node.score > 0.0 {
                    paths.push((path.clone(), score));
                }

                for child in &node.children {
                    let mut new_path = path.clone();
                    new_path.push(child.letters);
                    collect_paths_inner(child, new_path, child.score, paths);
                }
            }

            // Start the recursive traversal
            collect_paths_inner(&dyn_node, Vec::new(), dyn_node.score, &mut dyn_paths);

            // Now verify each path exists in the static trie with same score
            for (path, expected_score) in dyn_paths {
                let mut current_idx = 0; // Start at root

                // Follow the path in the static trie
                let mut found = true;
                for &letter in &path {
                    match static_trie.find_child(current_idx, letter) {
                        Some(next_idx) => {
                            current_idx = next_idx;
                        }
                        None => {
                            found = false;
                            break;
                        }
                    }
                }

                // Check that we found the path and the score matches
                assert!(
                    found,
                    "Path {:?} not found in static trie for key {:?}",
                    path, key
                );
                assert_eq!(
                    static_trie.nodes[current_idx].score, expected_score,
                    "Score mismatch for path {:?} in key {:?}",
                    path, key
                );
            }
        }
    }
}
