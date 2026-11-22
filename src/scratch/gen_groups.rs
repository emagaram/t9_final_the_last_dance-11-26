use rayon::prelude::*;
use std::collections::HashSet;

use crate::evaluation::evaluate::new_evaluate;
use crate::general::freq_list::create_freq_list64;
use crate::general::set32::Set32;
use crate::globals::NUM_LETTERS;
use crate::util::{set32s_to_string, ALPHABET_SET32};

/// Generate keyboard groups similar to the Python `gen_groups.py` script.
/// Uses `new_evaluate` as the scoring function (stop_at threshold).
pub fn gen_groups(dict_path: &str, num_cores: usize, stop_at: f32) -> Vec<Vec<(Vec<Set32>, f32)>> {
    // Load frequency list
    let freq_list = create_freq_list64(dict_path)
        .into_iter()
        .map(|(w, s)| (w, s as f32))
        .collect::<Vec<_>>();

    // Base letters as singleton Set32s
    let alphabet: Vec<Set32> = ALPHABET_SET32
        .ones_indices()
        .into_iter()
        .map(|i| Set32::singleton(i))
        .collect();

    // result[i] holds combinations of size i+1 and their scores
    let mut result: Vec<Vec<(Vec<Set32>, f32)>> = Vec::new();

    // Layer 1: single letters, score 0
    let layer1 = alphabet.iter().map(|&c| (vec![c], 0.0)).collect();
    result.push(layer1);

    for i in 1..NUM_LETTERS {
        let prev_layer = &result[i - 1];
        let prev_keys: HashSet<Vec<Set32>> = prev_layer.iter().map(|(k, _)| k.clone()).collect();

        // Generate next-layer combinations by appending larger letters
        let mut combos: Vec<Vec<Set32>> = Vec::new();
        for (config, _) in prev_layer {
            // find last bit index to enforce ordering
            if let Some(last) = config.last() {
                let last_idx = last.0.trailing_zeros();
                for &c in alphabet
                    .iter()
                    .skip_while(|&&l| l.0.trailing_zeros() <= last_idx)
                {
                    let mut new_cfg = config.clone();
                    new_cfg.push(c);
                    combos.push(new_cfg);
                }
            }
        }

        // Parallel scoring of this layer
        let scored: Vec<(Vec<Set32>, f32)> = combos
            .par_chunks((combos.len() + num_cores - 1) / num_cores)
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .filter_map(|config| {
                        // Check predecessor subsets exist
                        for idx in 0..config.len() {
                            let mut subset = config.clone();
                            subset.remove(idx);
                            if !prev_keys.contains(&subset) {
                                return None;
                            }
                        }
                        println!("Evaluating: {:?}", set32s_to_string(config));
                        let (score, _) = new_evaluate(&freq_list, config, stop_at);
                        if score > stop_at {
                            None
                        } else {
                            Some((config.clone(), score))
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        // Sort combinations by score (ascending)
        let mut scored_sorted = scored;
        scored_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        result.push(scored_sorted);
    }

    result
}
