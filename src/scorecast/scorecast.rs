use dashmap::DashMap;
use std::cmp::min;
use std::collections::HashMap;
use std::fs::{rename, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use tracing::{span, Level};

use crate::general::set32::Set32;
use crate::globals::NUM_LETTERS;
use crate::settings::SCORECAST_THREAD_NUMS;
use crate::util::ALPHABET_SET32;
use bincode::{config, Decode, Encode};

use rayon::prelude::*;

#[derive(Debug, Clone, Encode, Decode)]
pub struct ScorecastEncodeable {
    /// Given a Set32 with num of current keys equal to last completed layers, returns the smallest score for that Set32 assuming target_num_keys as the final number of keys.
    pub partial_estimates_target_num_keys: HashMap<Set32, f32>,
    pub pathkey_scores_by_layer: Vec<HashMap<Set32, f32>>, // Base scores for each layer, used to create new scores
    pub target_num_keys: usize,
    pub current_pathkey_scores: HashMap<Set32, f32>, // Current base scores to update base_scores
}

impl ScorecastEncodeable {
    pub fn convert_to_scorecast(self) -> Scorecast {
        Scorecast {
            partial_estimates_target_num_keys: self.partial_estimates_target_num_keys,
            pathkey_scores_by_layer: self.pathkey_scores_by_layer,
            target_num_keys: self.target_num_keys,
            current_pathkey_scores: self.current_pathkey_scores.clone().into_iter().collect(),
        }
    }
    pub fn from_scorecast(scorecast: &Scorecast) -> Self {
        Self {
            partial_estimates_target_num_keys: scorecast.partial_estimates_target_num_keys.clone(),
            pathkey_scores_by_layer: scorecast.pathkey_scores_by_layer.clone(),
            target_num_keys: scorecast.target_num_keys,
            current_pathkey_scores: scorecast
                .current_pathkey_scores
                .clone()
                .into_iter()
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scorecast {
    /// Given a Set32 with num of current keys equal to last completed layers, returns the smallest score for that Set32 assuming target_num_keys as the final number of keys.
    pub partial_estimates_target_num_keys: HashMap<Set32, f32>,
    /// Same as partial estimates but num of current keys is higher to allow each completed layer to not contribute to the key count.
    pub pathkey_scores_by_layer: Vec<HashMap<Set32, f32>>, // Base scores for each layer, used to create new scores
    pub target_num_keys: usize,
    current_pathkey_scores: DashMap<Set32, f32>, // Current base scores to update base_scores
}
impl Scorecast {
    pub fn new(target_num_keys: usize) -> Self {
        Self {
            partial_estimates_target_num_keys: HashMap::new(),
            pathkey_scores_by_layer: vec![],
            target_num_keys,
            current_pathkey_scores: DashMap::new(),
        }
    }

    pub fn save(&self, path: &str) {
        let file = tempfile::Builder::new()
            .tempfile_in(Path::new(path).parent().unwrap())
            .expect("Unable to create temp file");
        let mut writer = BufWriter::new(&file);
        let data = ScorecastEncodeable::from_scorecast(self);
        bincode::encode_into_std_write(data, &mut writer, config::standard()).unwrap();
        file.as_file().sync_all().expect("Unable to sync temp file");
        let path = Path::new(path);
        if !path.is_dir() {
            std::fs::create_dir_all(path.parent().unwrap())
                .expect("Unable to create scorecasts directory");
        }
        rename(file.path(), path).expect("Unable to rename temp file to scorecast file");
    }

    pub fn read(path: &str) -> Result<Scorecast, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let data: ScorecastEncodeable =
            bincode::decode_from_std_read(&mut reader, config::standard()).unwrap();
        Ok(data.convert_to_scorecast())
    }

    pub fn update_current_base_scores(&self, key: Set32, score: f32) {
        self.current_pathkey_scores
            .entry(key)
            .and_modify(|existing_score| {
                *existing_score = existing_score.min(score);
            })
            .or_insert(score);
        debug_assert_ne!(
            key,
            Set32::EMPTY,
            "Key for current base scores cannot be empty"
        );
    }

    pub fn get_score_target_num_keys(&self, keys: Set32) -> Option<f32> {
        let other = ALPHABET_SET32.difference(keys);
        let res = self.partial_estimates_target_num_keys.get(&other).copied();
        res
    }

    pub fn update(&mut self, threshold: f32) {
        let span_set_children = span!(Level::DEBUG, "Updating scorecast");
        let _enter_children = span_set_children.enter();
        self.update_pathkey_scores_by_layer();
        assert!(self.pathkey_scores_by_layer.len() < self.target_num_keys);
        self.partial_estimates_target_num_keys = self.generate_partial_estimates(
            threshold,
            self.target_num_keys - self.pathkey_scores_by_layer.len(),
        );
    }

    fn update_pathkey_scores_by_layer(&mut self) {
        let span_update_pathkey = span!(Level::DEBUG, "Updating pathkey scores by layer");
        let _enter_children = span_update_pathkey.enter();
        let mut current_base_scores_hm: HashMap<Set32, f32> = self
            .current_pathkey_scores
            .iter()
            .map(|item| (*item.key(), *item.value()))
            .collect();

        Scorecast::add_single_letters_to_base_score(
            &self.pathkey_scores_by_layer,
            self.pathkey_scores_by_layer.len(),
            &mut current_base_scores_hm,
        );

        self.pathkey_scores_by_layer.push(current_base_scores_hm);
        self.current_pathkey_scores.clear();
    }

    pub fn generate_partial_estimates(
        &mut self,
        threshold: f32,
        target_num_keys: usize,
    ) -> HashMap<Set32, f32> {
        Self::build_hm_par_from_base_scores(
            &self.pathkey_scores_by_layer,
            target_num_keys,
            threshold,
        )
    }
    // Take previous base_scores and union them with single letters such that the number of keys they represent equals the base score we are trying to add.
    // We have to do this because we don't use single letters in our solution tree.
    // Todo we can make indexes of keys that don't contain certain single letters and use them to speed this up. Probably not needed though.
    fn add_single_letters_to_base_score(
        pathkey_scores_by_layer: &[HashMap<Set32, f32>],
        current_pathkey_score_target_layer_idx: usize,
        current_pathkey_scores: &mut HashMap<Set32, f32>,
    ) {
        assert!(current_pathkey_score_target_layer_idx == pathkey_scores_by_layer.len());
        if current_pathkey_score_target_layer_idx == 0 {
            for key in ALPHABET_SET32.subsets_of_size(1 as u32) {
                assert_eq!(current_pathkey_scores.contains_key(&key), false);
                current_pathkey_scores.insert(key, 0.0);
            }
            return;
        }
        let prev_layer = &pathkey_scores_by_layer[current_pathkey_score_target_layer_idx - 1];
        for letter in ALPHABET_SET32.subsets_of_size(1) {
            for (&key, &value) in prev_layer.iter() {
                if !key.intersect(letter).is_empty() {
                    continue;
                }
                let new_key = key.union(letter);
                current_pathkey_scores
                    .entry(new_key)
                    .and_modify(|existing_score| {
                        *existing_score = existing_score.min(value);
                    })
                    .or_insert(value);
            }
        }
    }

    fn build_hm_par_from_base_scores(
        pathkey_scores_by_layer: &[HashMap<Set32, f32>],
        target_layer_count: usize,
        threshold: f32,
    ) -> HashMap<Set32, f32> {
        fn dfs(
            node_solution: Set32,
            nodes_solution: &[f32],
            depth: usize,
            visited_vecs: &mut [Vec<(Set32, f32)>],
            result: &mut Vec<f32>,
            threshold: f32,
        ) {
            let next_depth = depth + 1;
            let (first, second) = visited_vecs.split_at_mut(next_depth);
            let visited: &Vec<(Set32, f32)> = &first[depth];
            let new_visited = &mut second[0];
            let mut node_children = Vec::new();
            let node_score = nodes_solution[node_solution.0 as usize];
            // println!("DFS: Node solution: {}", set32_to_string(node_solution));
            let range = 0..node_solution.min_item().unwrap_or(NUM_LETTERS as u32);
            // println!("Range for DFS: {:?}", range);
            for i in range {
                let child = node_solution.add(i);
                if !nodes_solution[child.0 as usize].is_nan() {
                    node_children.push(child);
                }
            }
            if !node_children.is_empty() {
                new_visited.clear();
            }

            for (key, value) in visited {
                if !node_solution.intersect(*key).is_empty() {
                    continue;
                }
                let sum = node_score + value;
                if sum > threshold {
                    break;
                }
                let union = node_solution.union(*key);
                if !node_children.is_empty() {
                    new_visited.push((*key, *value));
                }
                let u = result[union.0 as usize];
                if u.is_nan() || u > sum {
                    result[union.0 as usize] = sum;
                }
            }

            for child in node_children {
                dfs(
                    child,
                    &nodes_solution,
                    next_depth,
                    visited_vecs,
                    result,
                    threshold,
                );
            }
        }
        let span_build_scorecast_par = span!(Level::DEBUG, "Updating pathkey scores by layer");
        let _enter_children = span_build_scorecast_par.enter();
        let mut result = vec![f32::NAN; 1 << NUM_LETTERS];
        let mut pathkey_scores_by_layer_sorted: Vec<Vec<(Set32, f32)>> = Vec::new();
        for map in pathkey_scores_by_layer {
            let mut next = Vec::with_capacity(map.len());
            for (&key, &value) in map.iter() {
                next.push((key, value));
            }
            next.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            pathkey_scores_by_layer_sorted.push(next);
        }

        let max_depth = NUM_LETTERS + 1;
        let mut total_layers_added = 0;
        let mut result_empty = true;
        let mut layers_remaining = target_layer_count;

        while layers_remaining > 0 {
            let span_layers_set = span!(
                Level::DEBUG,
                "Adding layer",
                layers_remaining = layers_remaining,
                total_layers_added = total_layers_added
            );
            let _enter_children = span_layers_set.enter();
            let layers_to_subtract = min(layers_remaining, pathkey_scores_by_layer_sorted.len());
            let layer_to_compare = &pathkey_scores_by_layer_sorted[layers_to_subtract - 1];
            if result_empty {
                for (key, val) in layer_to_compare {
                    result[key.0 as usize] = *val;
                }
                result_empty = false;
                layers_remaining -= layers_to_subtract;
                total_layers_added += layers_to_subtract;
                continue;
            }
            let curr_solution = result.clone();

            let starters: Vec<Set32> = ALPHABET_SET32
                .subsets_of_size(total_layers_added as u32)
                .collect();

            let thread_nums = SCORECAST_THREAD_NUMS;
            let thread_count =
                thread_nums[pathkey_scores_by_layer.len().min(thread_nums.len() - 1)];
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(thread_count)
                .build()
                .unwrap();
            pool.install(|| {
                // Parallel processing of starters
                let next = starters
                    .par_iter()
                    .fold(
                        || {
                            // Initialize both thread-local structures once per thread
                            let thread_result = vec![f32::NAN; 1 << NUM_LETTERS];
                            let mut thread_visited_vecs = vec![Vec::new(); max_depth];
                            // Initial copy of visited data
                            thread_visited_vecs[0] = layer_to_compare.clone();
                            (thread_result, thread_visited_vecs)
                        },
                        |(mut thread_result, mut thread_visited_vecs), starter| {
                            // Just reset visited data for each new starter

                            dfs(
                                *starter,
                                &curr_solution,
                                0,
                                &mut thread_visited_vecs,
                                &mut thread_result,
                                threshold,
                            );

                            (thread_result, thread_visited_vecs)
                        },
                    )
                    .map(|(thread_result, _)| thread_result)
                    .reduce(
                        || vec![f32::NAN; 1 << NUM_LETTERS], // Identity for reduction
                        |mut acc, thread_result| {
                            // Merge results by taking minimum values (excluding NaN)
                            for (acc_val, thread_val) in acc.iter_mut().zip(thread_result.iter()) {
                                if !thread_val.is_nan() {
                                    if acc_val.is_nan() || *thread_val < *acc_val {
                                        *acc_val = *thread_val;
                                    }
                                }
                            }
                            acc
                        },
                    );
                result = next;
            });
            layers_remaining -= layers_to_subtract;
            total_layers_added += layers_to_subtract;
        }

        result
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| {
                if !v.is_nan() {
                    Some((Set32(i as u32), v))
                } else {
                    None
                }
            })
            .collect()
    }
}

mod tests {
    #![allow(unused_imports)]
    use core::assert_eq;

    use crate::scorecast::scorecast_old::ScorecastOld;

    use super::*;

    #[cfg(test)]
    // RUSTFLAGS="-Awarnings" cargo test --release test_vector_vs_hashmap_performance -- --nocapture
    #[test]
    fn test_vector_vs_hashmap_performance() {
        use std::collections::HashMap;
        use std::io::BufReader;
        use std::time::Instant;

        // Create test data: some random Set32 values
        let test_size = 100_000_000;
        let mut test_keys = Vec::with_capacity(test_size);

        for _ in 0..test_size {
            let random_value = Set32(rand::random::<u32>() % (1 << NUM_LETTERS));
            test_keys.push(random_value);
        }

        // Test vector insertion
        let start = Instant::now();
        let mut vec_storage = vec![f32::INFINITY; 1 << NUM_LETTERS];
        for key in &test_keys {
            vec_storage[key.0 as usize] = 1.0;
        }
        let vec_insert_time = start.elapsed();

        // Test hashmap insertion
        let start = Instant::now();
        let mut map_storage = HashMap::with_capacity(test_size);
        for key in &test_keys {
            map_storage.insert(*key, 1.0);
        }
        let map_insert_time = start.elapsed();

        // Test vector retrieval
        let start = Instant::now();
        let mut vec_sum = 0.0;
        for key in &test_keys {
            vec_sum += vec_storage[key.0 as usize];
        }
        let vec_retrieval_time = start.elapsed();

        // Test hashmap retrieval
        let start = Instant::now();
        let mut map_sum = 0.0;
        for key in &test_keys {
            if let Some(val) = map_storage.get(key) {
                map_sum += *val;
            }
        }
        let map_retrieval_time = start.elapsed();

        println!(
            "Vector size: {} elements, HashMap size: {} elements",
            vec_storage.len(),
            map_storage.len()
        );
        println!(
            "Insertion times - Vector: {:?}, HashMap: {:?}, Ratio: {:.2}x",
            vec_insert_time,
            map_insert_time,
            map_insert_time.as_nanos() as f64 / vec_insert_time.as_nanos() as f64
        );
        println!(
            "Retrieval times - Vector: {:?}, HashMap: {:?}, Ratio: {:.2}x",
            vec_retrieval_time,
            map_retrieval_time,
            map_retrieval_time.as_nanos() as f64 / vec_retrieval_time.as_nanos() as f64
        );

        assert_eq!(vec_sum, map_sum);
    }
}
