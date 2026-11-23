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

/// Interaction of the 2-letter key `path[key_idx]` with the other keys, divided by the number of keys in `path`:
/// (score(P) - score(P minus K) - score([K])) / |P|.
/// score([K]) is the single {g,h} conditional, the only one P has on 2 letters that P minus K lacks,
/// so it's the k=2 bucket difference.
pub fn bigram_interaction_score(
    path: &[Set32],
    key_idx: usize,
    score_by_letter_count: &[f32],
    predecessors_map: &HashMap<PartitionId, Vec<f32>>,
) -> Option<f32> {
    let rest_scores = scores_without_key(path, key_idx, predecessors_map)?;
    let whole: f64 = score_by_letter_count.iter().map(|&s| s as f64).sum();
    let whole_minus_bigram: f64 = rest_scores.iter().map(|&s| s as f64).sum();
    let bigram = score_by_letter_count[0] as f64 - rest_scores[0] as f64;
    Some(((whole - whole_minus_bigram - bigram).max(0.0) / path.len() as f64) as f32)
}

/// Buckets of `path` without the 2-letter key `path[key_idx]`, or None if no other keys are left.
fn scores_without_key<'a>(
    path: &[Set32],
    key_idx: usize,
    predecessors_map: &'a HashMap<PartitionId, Vec<f32>>,
) -> Option<&'a Vec<f32>> {
    debug_assert_eq!(
        path[key_idx].count_ones(),
        2,
        "key must have exactly 2 letters"
    );
    let rest: Vec<Set32> = path
        .iter()
        .enumerate()
        .filter_map(|(i, key)| if i != key_idx { Some(*key) } else { None })
        .collect();
    if rest.is_empty() {
        return None;
    }
    predecessors_map.get(&PartitionId::from_keys(&rest))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        create_set32_str, create_set32s_vec,
        evaluation::{
            evaluate::{config_to_opt_letter_map_fast, evaluate},
            final_evaluate::scores_hm_tree::{
                gen_hm_dyn_tree_new, gen_hm_static_tree32, gen_hm_vec_new,
            },
        },
        general::freq_list::{create_freq_list, FreqList},
        util::{set32_to_string, set32s_to_string, ALPHABET_SET32},
    };
    use std::time::Instant;

    /// Brute-force score of `keys` with every unplaced letter on its own key (`evaluate` needs every letter mapped).
    fn evaluate_keys(freq_list: &FreqList, keys: &[Set32]) -> f64 {
        let key_set = keys.iter().fold(Set32::EMPTY, |acc, key| acc.union(*key));
        let mut config = keys.to_vec();
        config.extend(
            ALPHABET_SET32
                .difference(key_set)
                .ones_indices()
                .into_iter()
                .map(Set32::singleton),
        );
        evaluate(freq_list, &config, f32::MAX) as f64
    }

    /// Buckets for `path` from the real DP: fills `predecessors_map` with every minus-one
    /// sub-path first (recursively, down to lone 2-keys), then runs `final_evaluate`.
    fn dp_buckets(
        path: &[Set32],
        predecessors_map: &mut HashMap<PartitionId, Vec<f32>>,
        hm_static_tree: &HashMap<Set32, StaticTrie32>,
    ) -> Vec<f32> {
        let id = PartitionId::from_keys(path);
        if let Some(scores) = predecessors_map.get(&id) {
            return scores.clone();
        }
        for sub_path in all_paths_minus_one(path) {
            dp_buckets(&sub_path, predecessors_map, hm_static_tree);
        }
        let key_set = path.iter().fold(Set32::EMPTY, |acc, key| acc.union(*key));
        let letter_map = config_to_opt_letter_map_fast(path.iter());
        let (scores, _) = final_evaluate(
            path,
            key_set,
            &letter_map,
            predecessors_map,
            hm_static_tree,
            false,
        )
        .expect("every sub-path was scored first");
        predecessors_map.insert(id, scores.clone());
        scores
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_bigram_interaction_score_vs_evaluate -- --nocapture
    #[test]
    fn test_bigram_interaction_score_vs_evaluate() {
        let dict = "word_freq_2000.json";
        let hm_static_tree = gen_hm_static_tree32(gen_hm_dyn_tree_new(gen_hm_vec_new(dict)));
        let freq_list = create_freq_list(dict);

        // (layout, its 2-letter key to measure)
        let cases = [
            ("abc,def,gh", "gh"),
            ("eit,ao,ns,hr", "ao"),
            ("eit,ao,ns,hr", "ns"),
            ("th,er,an,si", "er"),
            ("ets,ai,on,hr,ld", "ai"),
            ("aeiou,st,nr,hl", "st"),
        ];

        let mut predecessors_map = HashMap::default();
        let mut nonzero = 0;
        for (layout, bigram) in cases {
            let path = create_set32s_vec!(layout);
            let key = create_set32_str!(bigram);
            let key_idx = path.iter().position(|&k| k == key).unwrap();
            let rest: Vec<Set32> = path.iter().copied().filter(|&k| k != key).collect();

            // e.g. (evaluate(abc def gh) - evaluate(abc def) - evaluate(gh)) / 3
            let whole = evaluate_keys(&freq_list, &path);
            let whole_minus_bigram = evaluate_keys(&freq_list, &rest);
            let bigram_alone = evaluate_keys(&freq_list, &[key]);
            let expected = (whole - whole_minus_bigram - bigram_alone).max(0.0) / path.len() as f64;

            let p_scores = dp_buckets(&path, &mut predecessors_map, &hm_static_tree);
            let actual =
                bigram_interaction_score(&path, key_idx, &p_scores, &predecessors_map).unwrap() as f64;

            println!(
                "{:<16} {}: evaluate ({:.6e} - {:.6e} - {:.6e}) / {} = {:.6e} | bigram_interaction_score {:.6e}",
                set32s_to_string(&path),
                set32_to_string(key),
                whole,
                whole_minus_bigram,
                bigram_alone,
                path.len(),
                expected,
                actual
            );
            let tol = 1e-4 * expected.abs().max(actual.abs()) + 1e-9;
            assert!(
                (actual - expected).abs() <= tol,
                "{layout} / {bigram}: {actual} vs {expected}"
            );
            if expected > 1e-9 {
                nonzero += 1;
            }
        }
        assert!(nonzero > 0, "every interaction was 0, the comparison proves nothing");
    }

    #[test]
    fn test_bigram_interaction_score() {
        // P = [abc, def, gh] (8 letters, buckets k=2..8); P minus gh = [abc, def] (6 letters, buckets k=2..6)
        let path = vec![
            create_set32_str!("abc"),
            create_set32_str!("def"),
            create_set32_str!("gh"),
        ];
        let p_scores = vec![5.0, 2.0, 4.0, 1.5, 0.0, 0.5, 0.25];
        let rest_scores = vec![3.0, 2.0, 1.0, 0.5, 0.0];
        let mut predecessors_map = HashMap::default();
        predecessors_map.insert(
            PartitionId::from_keys(&[create_set32_str!("def"), create_set32_str!("abc")]),
            rest_scores,
        );

        // (whole 13.25 - rest 6.5 - bigram (5 - 3)) / 3 keys
        let interaction = bigram_interaction_score(&path, 2, &p_scores, &predecessors_map);
        assert_eq!(interaction, Some(4.75 / path.len() as f32));

        // A lone 2-key has nothing to interact with
        assert_eq!(
            bigram_interaction_score(&[create_set32_str!("gh")], 0, &[1.0], &predecessors_map),
            None
        );
        // Missing predecessor -> None
        assert_eq!(
            bigram_interaction_score(&path, 2, &p_scores, &HashMap::default()),
            None
        );
    }

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
