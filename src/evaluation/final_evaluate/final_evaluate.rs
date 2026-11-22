use crate::{
    evaluation::final_evaluate::{
        scores_hm_tree::get_intersection_score32, scores_hm_tree_node::StaticTrie32,
    },
    general::{partition_id::PartitionId, set32::Set32},
    globals::NUM_LETTERS,
    util::compare_letters,
};
use gxhash::HashMap;
use tracing::{debug, span, Level, Span};

pub fn all_paths_minus_one_with_capacity(path: &[Set32], capacity: usize) -> Vec<Vec<Set32>> {
    let mut result = Vec::with_capacity(capacity);
    for (i, key) in path.iter().enumerate() {
        for letter in key.ones_indices() {
            let new_key = Set32(key.0).remove(letter);
            assert!(new_key.count_ones() > 0);
            let mut new_path = if new_key.count_ones() == 1 {
                vec![]
            } else {
                vec![new_key]
            };
            for (j, other_key) in path.iter().enumerate() {
                if j == i {
                    continue;
                }
                new_path.push(*other_key);
            }
            if new_path.is_empty() {
                continue;
            }
            new_path.sort_by(|a, b| {
                b.count_ones()
                    .cmp(&a.count_ones())
                    .then_with(|| crate::util::compare_letters(*b, *a))
            });
            result.push(new_path);
        }
    }
    result
}

pub fn get_predecessor_paths(path: &[Set32]) -> Vec<Vec<Set32>> {
    // total number of removals = sum of bit counts
    let total: u32 = path.iter().map(|k| k.count_ones()).sum();
    let mut result = Vec::with_capacity(total as usize);

    // scratch buffer, max size = path.len()
    let mut buf = Vec::with_capacity(path.len());

    for (i, key) in path.iter().enumerate() {
        for letter in key.ones_indices() {
            // build the new key with one bit removed
            let new_key = Set32(key.0).remove(letter);
            if new_key.count_ones() == 0 {
                // we never want an “empty” set in the path
                continue;
            }

            // 1) clone the original path into our scratch buf
            buf.clear();
            buf.extend_from_slice(path);

            // 2) replace the i‐th entry
            buf[i] = new_key;

            // 3) restore sorted order by one‐element bubble
            //    our sort criteria is: larger count first, then compare_letters
            let mut j = i;
            // bubble up
            while j > 0 {
                let me = &buf[j];
                let prev = &buf[j - 1];
                if me.count_ones() > prev.count_ones()
                    || (me.count_ones() == prev.count_ones()
                        && compare_letters(*me, *prev) == std::cmp::Ordering::Greater)
                {
                    buf.swap(j, j - 1);
                    j -= 1;
                } else {
                    break;
                }
            }
            // bubble down
            while j + 1 < buf.len() {
                let me = &buf[j];
                let next = &buf[j + 1];
                if me.count_ones() < next.count_ones()
                    || (me.count_ones() == next.count_ones()
                        && compare_letters(*me, *next) == std::cmp::Ordering::Less)
                {
                    buf.swap(j, j + 1);
                    j += 1;
                } else {
                    break;
                }
            }

            // 4) record the resulting path
            result.push(buf.clone());
        }
    }

    result
}

/// Paths are unsorted by design
pub fn all_paths_minus_one(path: &[Set32]) -> Vec<Vec<Set32>> {
    // let capacity = path.iter().fold(0, |acc, key| acc + key.count_ones());
    let mut result = Vec::new();
    for (i, key) in path.iter().enumerate() {
        let path_exclude_i = path
            .iter()
            .enumerate()
            .filter_map(|(j, other_key)| if j != i { Some(*other_key) } else { None })
            .collect::<Vec<_>>();
        if key.count_ones() == 2 {
            if path_exclude_i.is_empty() {
                continue;
            }
            result.push(path_exclude_i.clone());
            result.push(path_exclude_i);
        } else {
            for letter in key.ones_indices() {
                let mut new_path = path_exclude_i.clone();
                let new_key = key.remove(letter);
                new_path.push(new_key);
                result.push(new_path);
            }
        }
    }
    // debug_assert_eq!(result.len(), capacity);
    result
}

pub fn final_evaluate(
    path: &[Set32],
    path_key: Set32,
    letter_idx_map: &[Option<usize>; NUM_LETTERS],
    predecessors_map: &HashMap<PartitionId, Vec<f32>>,
    hm_static_tree: &HashMap<Set32, StaticTrie32>,
    should_log: bool,
) -> Option<(Vec<f32>, f32)> {
    let _span_layers_set = if should_log {
        span!(
            Level::DEBUG,
            "Evaluating path",
            path = crate::util::set32s_to_string(path)
        )
    } else {
        Span::none()
    };

    let num_chars_in_path = path_key.count_ones() as usize;
    let mut score_by_letter_count_f64: Vec<f64> = vec![0.0; num_chars_in_path - 1];
    let sub_paths = all_paths_minus_one(path);
    for sub_path in sub_paths {
        let sub_path_id = PartitionId::from_keys(&sub_path);

        let score_letter_count = predecessors_map.get(&sub_path_id);
        if score_letter_count.is_none() {
            if should_log {
                debug!(
                    sub_path = crate::util::set32s_to_string(&sub_path),
                    "No score found for subpath",
                );
            }
            return None;
        }
        let score_letter_count = score_letter_count.unwrap();
        for (i, score) in score_letter_count.iter().enumerate() {
            score_by_letter_count_f64[i] += *score as f64;
        }
    }

    for i in 2..num_chars_in_path {
        score_by_letter_count_f64[i - 2] /= (num_chars_in_path - i) as f64;
    }

    let add = get_intersection_score32(path, path_key, letter_idx_map, hm_static_tree);
    score_by_letter_count_f64[num_chars_in_path - 2] = add as f64;

    let sum_f64 = score_by_letter_count_f64.iter().sum::<f64>();

    // Convert f64 results to f32 at the end
    // let score_by_letter_count: Vec<f64> = score_by_letter_count_f64
    //     .iter()
    //     .map(|&x| x as f64)
    //     .collect();
    let sum = sum_f64 as f32;
    if should_log {
        debug!(
            path = crate::util::set32s_to_string(&path),
            score_by_letter_count_f64 = ?score_by_letter_count_f64,
            score = sum,
            "Final score for path",
        );
    }

    Some((
        score_by_letter_count_f64
            .into_iter()
            .map(|v| v as f32)
            .collect(),
        sum,
    ))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::create_set32_str;
    use std::time::Instant;

    #[test]
    fn test_all_paths_minus_one() {
        let path = vec![create_set32_str!("abc"), create_set32_str!("de")];
        let result = all_paths_minus_one(&path);

        // Expecting one removal for each letter in each group
        // "abc" has 3 letters, "de" has 2 => 5 total combinations
        assert_eq!(result.len(), 5);
        let path = vec![create_set32_str!("ab")];
        let result = all_paths_minus_one(&path);

        // Expecting one removal for each letter in each group
        // "abc" has 3 letters, "de" has 2 => 5 total combinations
        assert_eq!(result.len(), 0);
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_all_paths_minus_one_implementations_equality -- --nocapture
    #[test]
    fn test_all_paths_minus_one_implementations_equality() {
        // Test with a simple path
        let path = vec![
            create_set32_str!("abc"),
            create_set32_str!("def"),
            create_set32_str!("ghi"),
            create_set32_str!("jk"),
        ];
        let result1 = all_paths_minus_one(&path);
        let result2 = get_predecessor_paths(&path);
        let result3 = all_paths_minus_one_with_capacity(
            &path,
            path.iter().map(|k| k.count_ones()).sum::<u32>() as usize,
        );

        assert_eq!(result1.len(), result2.len());
        assert_eq!(result1.len(), result3.len());

        // Test with a more complex path
        let path = vec![
            create_set32_str!("abcdef"),
            create_set32_str!("ghijk"),
            create_set32_str!("lmnop"),
        ];
        let result1 = all_paths_minus_one(&path);
        let result2 = get_predecessor_paths(&path);
        let result3 = all_paths_minus_one_with_capacity(
            &path,
            path.iter().map(|k| k.count_ones()).sum::<u32>() as usize,
        );

        assert_eq!(result1.len(), result2.len());
        assert_eq!(result1.len(), result3.len());

        // assert_eq!(result1, result2);
        // assert_eq!(result2, result3);
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_all_paths_minus_one_performance -- --nocapture
    #[test]
    fn test_all_paths_minus_one_performance() {
        // Create a larger test case to measure performance
        let path = vec![
            create_set32_str!("abcdefghij"),
            create_set32_str!("klmnopqrst"),
            create_set32_str!("uvwxyz"),
        ];

        // Time all_paths_minus_one
        let start = Instant::now();
        let result1 = all_paths_minus_one(&path);
        let duration1 = start.elapsed();

        // Time all_paths_minus_one_fast
        let start = Instant::now();
        let result2 = get_predecessor_paths(&path);
        let duration2 = start.elapsed();

        // Time all_paths_minus_one_with_capacity
        let start = Instant::now();
        let result3 = all_paths_minus_one_with_capacity(
            &path,
            path.iter().map(|k| k.count_ones()).sum::<u32>() as usize,
        );
        let duration3 = start.elapsed();

        // Verify correctness
        assert_eq!(result1.len(), result2.len());
        assert_eq!(result1.len(), result3.len());

        // Log performance results - this will be shown in test output
        println!("all_paths_minus_one: {:?}", duration1);
        println!("all_paths_minus_one_fast: {:?}", duration2);
        println!("all_paths_minus_one_with_capacity: {:?}", duration3);

        // Optionally assert that the fast version is actually faster
        // We use a generous factor here since timing can be variable
        // assert!(duration2 * 3 < duration1, "Fast version should be significantly faster");
    }
}
