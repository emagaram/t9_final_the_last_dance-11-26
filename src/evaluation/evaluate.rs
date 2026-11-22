use crate::{
    general::set32::Set32,
    globals::NUM_LETTERS,
    util::{set32s_to_string, ALPHABET_SET32},
};
use fxhash::FxHashMap;
use gxhash::{HashMap, HashMapExt};
use rayon::prelude::*;
use std::{cell::RefCell, collections::hash_map::Entry};

// const ALPHABET_CONFIG: [usize; NUM_LETTERS] = {
//     let mut arr = [0; NUM_LETTERS];
//     let mut i = 0;
//     while i < NUM_LETTERS {
//         arr[i] = i;
//         i += 1;
//     }
//     arr
// };

pub fn get_alphabet_set(a: &Set32) -> Set32 {
    a.intersect(ALPHABET_SET32)
}
fn config_to_t10_config_full(config: &Vec<Set32>, config_hashmap: &mut HashMap<u8, u8>) {
    for (config_index, set) in config.iter().enumerate() {
        let key_indices = set.ones_indices();
        for key_index in key_indices {
            config_hashmap.insert(
                key_index.try_into().unwrap(),
                config_index.try_into().unwrap(),
            );
        }
    }
}

fn word_to_t10(word: &Vec<u8>, t10_config: &HashMap<u8, u8>) -> Vec<u8> {
    word.iter()
        .map(|b| *t10_config.get(&b).expect("Should have a key"))
        .collect()
}

fn word_to_t10_arr(word: &[u8], t10_config: &[u8; NUM_LETTERS]) -> Vec<u8> {
    word.iter()
        .map(|&b| unsafe { *t10_config.get_unchecked(b as usize) })
        .collect()
}

fn word_to_t10_idx<T>(word: &[u8], t10_config: &[u8; NUM_LETTERS]) -> T
where
    T: From<u8> + std::ops::Mul<Output = T> + std::ops::Add<Output = T> + Copy,
{
    // Remove the constant declaration that uses runtime values
    let mut result = T::from(0);
    let num_letters = T::from(NUM_LETTERS as u8);
    for &b in word {
        // Multiply by the base (number of letters)
        result = result * num_letters;
        // Add the value plus 1 (to avoid 0 which could cause ambiguity)
        let key_value = unsafe { T::from(*t10_config.get_unchecked(b as usize) + 1) };
        result = result + key_value;
    }
    result
}

pub fn new_evaluate(
    freq_list: &Vec<(Vec<u8>, f32)>,
    config: &Vec<Set32>,
    stop_at: f32,
) -> (f32, usize) {
    let unused_letters = get_missing_letters(config);
    let num_keys = config.len() + unused_letters.len();
    let mut full_config = config.clone();
    full_config.extend(&unused_letters);
    let base_score = evaluate(freq_list, &full_config, stop_at);
    return (base_score, num_keys);
}

thread_local! {
    static COUNT_HM: RefCell<HashMap<Vec<u8>, u32>> = RefCell::new(HashMap::new());
}
pub fn new_evaluate_arr(
    freq_list: &[(Vec<u8>, f32)],
    config: &Vec<Set32>,
    stop_at: f32,
) -> (f32, usize) {
    let unused_letters = get_missing_letters(config);
    let num_keys = config.len() + unused_letters.len();
    let mut full_config = config.clone();
    full_config.extend(&unused_letters);

    // Use the thread local COUNT_HM
    let base_score = COUNT_HM.with(|count_hm_cell| {
        let mut count_hm = count_hm_cell.borrow_mut();
        count_hm.clear();
        evaluate_arr(freq_list, &full_config, &mut count_hm, stop_at)
    });

    return (base_score, num_keys);
}

pub fn config_to_opt_letter_map_fast<'a, T>(config: T) -> [Option<usize>; NUM_LETTERS]
where
    T: Iterator<Item = &'a Set32>,
{
    let mut output: [Option<usize>; NUM_LETTERS] = [None; NUM_LETTERS]; // Initialize array with None

    // First pass: set all known keys
    for (i, key) in config.enumerate() {
        let key_indices = key.ones_indices();
        for key_index in key_indices {
            output[key_index as usize] = Some(i as usize);
        }
    }
    output
}

// pub fn config_to_letter_map_fast<'a, T>(config: T) -> [usize; NUM_LETTERS]
// where
//     T: Iterator<Item = &'a Set32>,
// {
//     let mut output: [usize; NUM_LETTERS] = ALPHABET_CONFIG; // Initialize array with zeros

//     // First pass: set all known keys
//     for (i, key) in config.enumerate() {
//         let key_indices = key.ones_indices();
//         for key_index in key_indices {
//             output[key_index as usize] = i as usize;
//         }
//     }
//     output
// }

fn config_to_10_arr(config: &Vec<Set32>) -> [u8; NUM_LETTERS] {
    let mut output: [u8; NUM_LETTERS] = [0; NUM_LETTERS]; // Initialize array with zeros

    // Track which indices we've seen so far
    let mut seen = Set32::EMPTY;

    // First pass: set all known keys
    for (i, key) in config.iter().enumerate() {
        let key_indices = key.ones_indices();
        for key_index in key_indices {
            output[key_index as usize] = i as u8;
            seen = seen.add(key_index);
        }
    }

    // Second pass: handle unseen indices
    let unseen = get_alphabet_set(&Set32(!seen.value()));
    let unseen_indices = unseen.ones_indices();
    let config_len = config.len();

    for (i, key_index) in unseen_indices.into_iter().enumerate() {
        output[key_index as usize] = (config_len + i) as u8;
    }
    output
}

pub fn config_to_10_arr_size(config: &Vec<Set32>) -> ([u8; NUM_LETTERS], usize) {
    let mut output: [u8; NUM_LETTERS] = [0; NUM_LETTERS]; // Initialize array with zeros

    // Track which indices we've seen so far
    let mut seen = Set32::EMPTY;

    // First pass: set all known keys
    for (i, key) in config.iter().enumerate() {
        let key_indices = key.ones_indices();
        for key_index in key_indices {
            output[key_index as usize] = i as u8;
            seen = seen.add(key_index);
        }
    }

    // Second pass: handle unseen indices
    let unseen = get_alphabet_set(&Set32(!seen.value()));
    let unseen_indices = unseen.ones_indices();
    let config_len = config.len();
    let unseen_len = unseen_indices.len();

    for (i, key_index) in unseen_indices.into_iter().enumerate() {
        output[key_index as usize] = (config_len + i) as u8;
    }
    (output, config_len + unseen_len)
}

pub fn evaluate_arr_int(
    freq_list: &[(Vec<u8>, u64)],
    config: &Vec<Set32>,
    count_hm: &mut HashMap<Vec<u8>, u64>,
    stop_at: u64,
) -> u64 {
    let config_arr = config_to_10_arr(&config);
    let mut score = 0;

    for (word, value) in freq_list {
        let t10_word = word_to_t10_arr(word, &config_arr);
        let count = count_hm.entry(t10_word).or_insert(0);
        if *count != 0 {
            score += value * *count;
            if score > stop_at {
                return score;
            }
        }
        *count = *count + 1;
    }
    return score;
}

pub fn evaluate_arr_opt_timed(
    freq_lists: &[(u8, Vec<u8>)],
    config: &Vec<Set32>,
    count_arr: &mut [(u8, usize)],
    count_hm_64: &mut FxHashMap<u64, u8>,
    count_hm_128: &mut FxHashMap<u128, u8>,
    count_hm_vec: &mut FxHashMap<Vec<u8>, u8>,
    stop_at: f32,
    iteration_id: usize,
) -> f32 {
    // Timing variables
    let time_config_setup;
    let mut time_array_words = std::time::Duration::ZERO;
    let mut time_u64_words = std::time::Duration::ZERO;
    let mut time_u128_words = std::time::Duration::ZERO;
    let mut time_vec_words = std::time::Duration::ZERO;

    // Configuration setup
    let start = std::time::Instant::now();
    let (mut config_arr, config_size) = config_to_10_arr_size(&config);
    for c in config_arr.iter_mut() {
        *c += 1;
    }
    time_config_setup = start.elapsed();

    let mut score = 0.0;
    // Calculate maximum word length that can be represented in each type
    let max_idx_len = count_arr.len().ilog(config_size + 1) as u8;
    let max_u64_len = u64::MAX.ilog((config_size + 1) as u64) as u8;
    let max_u128_len = u128::MAX.ilog((config_size + 1) as u128) as u8;

    for (len, freq_list) in freq_lists {
        let len_size = *len as usize;
        let inc = len_size + 4;

        // Case 1: Words that fit in array index
        if *len <= max_idx_len {
            let start = std::time::Instant::now();
            let conf_size = config_size as u64;
            for chunk in freq_list.chunks_exact(inc) {
                // Calculate word index
                let mut word_idx: u64 = 0;
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u64 };
                }

                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) };

                // Access array without bounds checking
                let val = unsafe { count_arr.get_unchecked_mut(word_idx as usize) };
                if val.1 != iteration_id {
                    val.1 = iteration_id;
                    val.0 = 1;
                } else {
                    score += word_score * val.0 as f32;
                    if score > stop_at {
                        println!(
                            "Timing: config={:?}, array={:?}, u64={:?}, u128={:?}, vec={:?}",
                            time_config_setup,
                            time_array_words,
                            time_u64_words,
                            time_u128_words,
                            time_vec_words
                        );
                        return score;
                    }
                    val.0 += 1;
                }
            }
            time_array_words += start.elapsed();
        }
        // Case 2: Words that fit in u64 but not in array index
        else if *len <= max_u64_len {
            let start = std::time::Instant::now();
            let conf_size = config_size as u64;
            for chunk in freq_list.chunks_exact(inc) {
                // Calculate word index
                let mut word_idx: u64 = 0;
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u64 };
                }

                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) };

                match count_hm_64.entry(word_idx) {
                    Entry::Occupied(mut e) => {
                        let val = e.get_mut();
                        score += word_score * *val as f32;
                        if score > stop_at {
                            println!(
                                "Timing: config={:?}, array={:?}, u64={:?}, u128={:?}, vec={:?}",
                                time_config_setup,
                                time_array_words,
                                time_u64_words,
                                time_u128_words,
                                time_vec_words
                            );
                            return score;
                        }
                        *val += 1;
                    }
                    Entry::Vacant(e) => {
                        e.insert(1);
                    }
                }
            }
            time_u64_words += start.elapsed();
        }
        // Case 3: Words that fit in u128 but not in u64
        else if *len <= max_u128_len {
            let start = std::time::Instant::now();
            let conf_size = config_size as u128;
            for chunk in freq_list.chunks_exact(inc) {
                // Calculate word index
                let mut word_idx: u128 = 0;
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u128 };
                }

                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) };

                match count_hm_128.entry(word_idx) {
                    Entry::Occupied(mut e) => {
                        let val = e.get_mut();
                        score += word_score * *val as f32;
                        if score > stop_at {
                            println!(
                                "Timing: config={:?}, array={:?}, u64={:?}, u128={:?}, vec={:?}",
                                time_config_setup,
                                time_array_words,
                                time_u64_words,
                                time_u128_words,
                                time_vec_words
                            );
                            return score;
                        }
                        *val += 1;
                    }
                    Entry::Vacant(e) => {
                        e.insert(1);
                    }
                }
            }
            time_u128_words += start.elapsed();
        }
        // Case 4: Words that don't fit in u128
        else {
            let start = std::time::Instant::now();
            for chunk in freq_list.chunks_exact(inc) {
                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) };

                // Create word vector
                let mut word_vec = Vec::with_capacity(len_size);
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_vec.push(unsafe { *config_arr.get_unchecked(byte as usize) });
                }

                let val = count_hm_vec.entry(word_vec).or_insert(0);
                if *val != 0 {
                    score += word_score * *val as f32;
                    if score > stop_at {
                        println!(
                            "Timing: config={:?}, array={:?}, u64={:?}, u128={:?}, vec={:?}",
                            time_config_setup,
                            time_array_words,
                            time_u64_words,
                            time_u128_words,
                            time_vec_words
                        );
                        return score;
                    }
                }
                *val += 1;
            }
            time_vec_words += start.elapsed();
        }
    }

    println!(
        "Timing: config={:?}, array={:?}, u64={:?}, u128={:?}, vec={:?}",
        time_config_setup, time_array_words, time_u64_words, time_u128_words, time_vec_words
    );
    return score;
}

fn process_hash_entry<K>(
    entry: Entry<K, u8>,
    score: &mut f64,
    word_score: f64,
    stop_at: f64,
    num_keys: usize,
) -> Option<(f32, u8)> {
    match entry {
        Entry::Occupied(mut e) => {
            let val = e.get_mut();
            *score += word_score * *val as f64;
            if *score > stop_at {
                return Some((*score as f32, num_keys as u8));
            }
            *val += 1;
            None
        }
        Entry::Vacant(e) => {
            e.insert(1);
            None
        }
    }
}

pub fn evaluate_arr_opt(
    freq_lists: &[(u8, Vec<u8>)],
    config: &Vec<Set32>,
    count_hm_64: &mut FxHashMap<u64, u8>,
    count_hm_128: &mut FxHashMap<u128, u8>,
    count_hm_vec: &mut FxHashMap<Vec<u8>, u8>,
    stop_at: f32,
) -> (f32, u8) {
    let (mut config_arr, num_keys) = config_to_10_arr_size(&config);

    for c in config_arr.iter_mut() {
        *c += 1;
    }
    for (i, c) in config_arr.iter().enumerate() {
        println!("config_arr[{}]: {}", i, c);
    }
    let stop_at = stop_at as f64;
    let mut score: f64 = 0.0;

    // Calculate maximum word length that can be represented in each type
    // let max_idx_len = count_arr.len().ilog(num_keys + 1) as u8;
    let max_u64_len = u64::MAX.ilog((num_keys + 1) as u64) as u8;
    let max_u128_len = u128::MAX.ilog((num_keys + 1) as u128) as u8;
    let conf_size = num_keys as u64;

    for (len, freq_list) in freq_lists {
        let len_size = *len as usize;
        let inc = len_size + 4;

        for chunk in freq_list.chunks_exact(inc) {
            // Get word score using unsafe pointer arithmetic
            let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
            let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) } as f64;
            let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

            if *len <= max_u64_len {
                let mut word_idx: u64 = 0;
                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u64 };
                }

                if let Some(result) = process_hash_entry(
                    count_hm_64.entry(word_idx),
                    &mut score,
                    word_score,
                    stop_at,
                    num_keys,
                ) {
                    return result;
                }
            } else if *len <= max_u128_len {
                let mut word_idx: u128 = 0;
                for &byte in word_bytes {
                    word_idx = word_idx * conf_size as u128
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u128 };
                }

                if let Some(result) = process_hash_entry(
                    count_hm_128.entry(word_idx as u128),
                    &mut score,
                    word_score,
                    stop_at,
                    num_keys,
                ) {
                    return result;
                }
            } else {
                // Vec<u8> implementation for very long words
                let mut word_vec = Vec::with_capacity(len_size);
                for &byte in word_bytes {
                    word_vec.push(unsafe { *config_arr.get_unchecked(byte as usize) });
                }
                if let Some(result) = process_hash_entry(
                    count_hm_vec.entry(word_vec),
                    &mut score,
                    word_score,
                    stop_at,
                    num_keys,
                ) {
                    return result;
                }
            }
        }
    }
    return (score as f32, num_keys as u8);
}
pub fn evaluate_arr_opt_old(
    freq_lists: &[(u8, Vec<u8>)],
    config: &Vec<Set32>,
    count_arr: &mut [(u8, usize)],
    count_hm_64: &mut FxHashMap<u64, u8>,
    count_hm_128: &mut FxHashMap<u128, u8>,
    count_hm_vec: &mut FxHashMap<Vec<u8>, u8>,
    stop_at: f32,
    iteration_id: usize,
) -> (f32, u8) {
    let (mut config_arr, num_keys) = config_to_10_arr_size(&config);
    for c in config_arr.iter_mut() {
        *c += 1;
    }
    let stop_at = stop_at as f64;
    let mut score: f64 = 0.0;
    // Calculate maximum word length that can be represented in each type
    let max_idx_len = count_arr.len().ilog(num_keys + 1) as u8;
    let max_u64_len = u64::MAX.ilog((num_keys + 1) as u64) as u8;
    let max_u128_len = u128::MAX.ilog((num_keys + 1) as u128) as u8;

    for (len, freq_list) in freq_lists {
        let len_size = *len as usize;
        let inc = len_size + 4;
        // Case 1: Words that fit in array index
        if false && *len <= max_idx_len {
            let conf_size = num_keys as u64;
            for chunk in freq_list.chunks_exact(inc) {
                // Calculate word index
                let mut word_idx: u64 = 0;
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u64 };
                }

                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) } as f64;

                // Access array without bounds checking
                let val = unsafe { count_arr.get_unchecked_mut(word_idx as usize) };
                if val.1 != iteration_id {
                    val.1 = iteration_id;
                    val.0 = 1;
                } else {
                    score += word_score * val.0 as f64;
                    if score > stop_at {
                        return (score as f32, num_keys as u8);
                    }
                    val.0 += 1;
                }
            }
        }
        // Case 2: Words that fit in u64 but not in array index
        else if *len <= max_u64_len {
            let conf_size = num_keys as u64;
            for chunk in freq_list.chunks_exact(inc) {
                // Calculate word index
                let mut word_idx: u64 = 0;
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u64 };
                }

                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) } as f64;

                match count_hm_64.entry(word_idx) {
                    Entry::Occupied(mut e) => {
                        let val = e.get_mut();
                        score += word_score * *val as f64;
                        if score > stop_at {
                            return (score as f32, num_keys as u8);
                        }
                        *val += 1;
                    }
                    Entry::Vacant(e) => {
                        e.insert(1);
                    }
                }
            }
        }
        // Case 3: Words that fit in u128 but not in u64
        else if *len <= max_u128_len {
            let conf_size = num_keys as u128;
            for chunk in freq_list.chunks_exact(inc) {
                // Calculate word index
                let mut word_idx: u128 = 0;
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { *config_arr.get_unchecked(byte as usize) as u128 };
                }

                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) } as f64;

                match count_hm_128.entry(word_idx) {
                    Entry::Occupied(mut e) => {
                        let val = e.get_mut();
                        score += word_score * *val as f64;
                        if score > stop_at {
                            return (score as f32, num_keys as u8);
                        }
                        *val += 1;
                    }
                    Entry::Vacant(e) => {
                        e.insert(1);
                    }
                }
            }
        }
        // Case 4: Words that don't fit in u128
        else {
            for chunk in freq_list.chunks_exact(inc) {
                // Get word score using unsafe pointer arithmetic
                let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
                let word_score = unsafe { *(score_bytes.as_ptr() as *const f32) } as f64;

                // Create word vector
                let mut word_vec = Vec::with_capacity(len_size);
                let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

                for &byte in word_bytes {
                    word_vec.push(unsafe { *config_arr.get_unchecked(byte as usize) });
                }

                let val = count_hm_vec.entry(word_vec).or_insert(0);
                if *val != 0 {
                    score += word_score * *val as f64;
                    if score > stop_at {
                        return (score as f32, num_keys as u8);
                    }
                }
                *val += 1;
            }
        }
    }
    return (score as f32, num_keys as u8);
}

pub fn evaluate_arr_idx(
    freq_list: &[(Vec<u8>, f32)],
    config: &Vec<Set32>,
    count_hm: &mut HashMap<u128, u32>,
    stop_at: f32,
) -> f32 {
    let config_arr = config_to_10_arr(&config);
    let mut score = 0.0;

    for (word, value) in freq_list {
        let t10_idx = word_to_t10_idx(word, &config_arr);

        // Time hashmap lookup and insertion
        let count = count_hm.entry(t10_idx).or_insert(0);

        if *count != 0 {
            score += value * *count as f32;

            if score > stop_at {
                return score;
            }
        }
        *count += 1;
    }
    return score;
}

pub fn evaluate_arr_idx_timed(
    freq_list: &[(Vec<u8>, f32)],
    config: &Vec<Set32>,
    count_hm: &mut HashMap<u128, u32>,
    stop_at: f32,
) -> f32 {
    let config_arr = config_to_10_arr(&config);
    let mut score = 0.0;

    // Timing variables
    let mut total_hash_time = std::time::Duration::ZERO;
    let mut total_mapping_time = std::time::Duration::ZERO;
    let mut total_score_time = std::time::Duration::ZERO;

    for (word, value) in freq_list {
        // Time word mapping
        let mapping_start = std::time::Instant::now();
        let t10_idx = word_to_t10_idx(word, &config_arr);
        total_mapping_time += mapping_start.elapsed();

        // Time hashmap lookup and insertion
        let hash_start = std::time::Instant::now();
        let count = count_hm.entry(t10_idx).or_insert(0);
        total_hash_time += hash_start.elapsed();

        if *count != 0 {
            // Time score calculation
            let score_start = std::time::Instant::now();
            score += value * *count as f32;
            total_score_time += score_start.elapsed();

            if score > stop_at {
                println!(
                    "Timing stats: mapping={}ms, hashing={}ms, scoring={}ms",
                    total_mapping_time.as_micros() as f64 / 1000.0,
                    total_hash_time.as_micros() as f64 / 1000.0,
                    total_score_time.as_micros() as f64 / 1000.0
                );
                return score;
            }
        }
        *count += 1;
    }

    println!(
        "Timing stats: mapping={}ms, hashing={}ms, scoring={}ms",
        total_mapping_time.as_micros() as f64 / 1000.0,
        total_hash_time.as_micros() as f64 / 1000.0,
        total_score_time.as_micros() as f64 / 1000.0
    );
    return score;
}

pub fn evaluate_arr(
    freq_list: &[(Vec<u8>, f32)],
    config: &Vec<Set32>,
    count_hm: &mut HashMap<Vec<u8>, u32>,
    stop_at: f32,
) -> f32 {
    let config_arr = config_to_10_arr(&config);
    let mut score = 0.0;

    for (word, value) in freq_list {
        // Time word mapping
        let t10_word = word_to_t10_arr(word, &config_arr);
        let count = count_hm.entry(t10_word).or_insert(0);

        if *count != 0 {
            score += value * *count as f32;

            if score > stop_at {
                return score;
            }
        }
        *count += 1;
    }
    return score;
}

pub fn evaluate(freq_list: &Vec<(Vec<u8>, f32)>, config: &Vec<Set32>, stop_at: f32) -> f32 {
    let mut config_hm: HashMap<u8, u8> = HashMap::new();
    let mut count_hm: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut score: f64 = 0.0;
    config_to_t10_config_full(&config, &mut config_hm);
    for (word, value) in freq_list {
        let t10_word = word_to_t10(word, &config_hm);
        let count = count_hm.entry(t10_word).or_insert(0);
        if *count != 0 {
            score += (*value as f64) * *count as f64;
            if score > stop_at as f64 {
                return score as f32;
            }
        }
        *count = *count + 1;
        // println!("Evaluating word: {}, score: {}", String::from_utf8_lossy(word), score);
    }
    return score as f32;
}

pub fn evaluate_full(
    freq_list: &Vec<(Vec<u8>, f32)>,
    config: &Vec<Set32>,
    stop_at: f32,
    target_num_keys: usize,
) -> (f32, usize, usize) {
    let unused_letters = get_missing_letters(config);
    let num_keys = config.len() + unused_letters.len();
    // println!("Config num keys: {}", num_keys);
    let variations = merge_slices(&config, &unused_letters);
    let mut scores: Vec<_> = variations
        .par_iter()
        .map(|variation| evaluate(freq_list, variation, stop_at))
        .collect();
    let num_merges_needed = num_keys - target_num_keys;
    // println!("Num merges needed: {}", num_keys);
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut full_config = config.clone();
    full_config.extend(&unused_letters);
    let base_score = evaluate(freq_list, &full_config, stop_at);

    let mut score_sum = base_score;
    for i in 0..num_merges_needed {
        let add = scores[i] - base_score;
        println!("Adding {} to {}", add, set32s_to_string(config));
        score_sum += add;
    }
    return (score_sum, num_keys, NUM_LETTERS - unused_letters.len());
}

fn get_missing_letters(config: &Vec<Set32>) -> Vec<Set32> {
    let mut missing_letters_set = Set32::EMPTY;
    let mut res = Vec::new();
    for key in config {
        missing_letters_set = missing_letters_set.union(*key);
    }
    missing_letters_set = get_alphabet_set(&Set32(!missing_letters_set.value()));
    for one in missing_letters_set.ones_indices() {
        println!("Missing letter: {}", one);
        res.push(Set32::singleton(one));
    }
    res
}

fn merge_slices(keys: &Vec<Set32>, unused_letters: &Vec<Set32>) -> Vec<Vec<Set32>> {
    let mut results = Vec::new();

    // Pair each item in `unused_letters` with each item in `keys`
    for (key_index, key) in keys.iter().enumerate() {
        for (let_index, unused) in unused_letters.iter().enumerate() {
            let mut merged = Vec::new();
            let mut new_keys = keys.clone();
            new_keys.remove(key_index);
            let merged_key = key.union(*unused);
            let mut new_unused = unused_letters.clone();
            new_unused.remove(let_index);
            merged.push(merged_key);
            merged.append(&mut new_unused);
            merged.append(&mut new_keys);
            results.push(merged);
        }
    }

    // Pair each item in `unused_letters` with every other item in `unused_letters`
    for i in 0..unused_letters.len() {
        for j in i + 1..unused_letters.len() {
            let mut merged = Vec::new();
            let merged_unused = unused_letters[i].union(unused_letters[j]);
            let mut new_unused = unused_letters.clone();
            new_unused.remove(i);
            new_unused.remove(j - 1);
            merged.push(merged_unused);
            merged.append(&mut keys.to_vec());
            merged.append(&mut new_unused);
            results.push(merged);
        }
    }
    results
}

#[cfg(test)]
mod tests {

    use fxhash::FxHashMap;
    use gxhash::{HashMap, HashMapExt};

    use super::{evaluate, evaluate_full, get_missing_letters};

    #[allow(unused_imports)]
    use crate::{
        create_set32, create_set32_str,
        evaluation::evaluate::{
            config_to_10_arr, config_to_t10_config_full, evaluate_arr, evaluate_arr_idx,
            evaluate_arr_int, evaluate_arr_opt, evaluate_arr_opt_timed, merge_slices,
            word_to_t10_arr, word_to_t10_idx,
        },
        general::freq_list::create_freq_list,
        general::set32::Set32,
        globals::CHAR_TO_INDEX,
        util::set32s_to_string,
    };
    use std::time::Instant;

    #[test]
    fn evaluate_full_best_10_key() {
        let freq_list: Vec<(Vec<u8>, f32)> = create_freq_list("./word_freq.json");
        let target = 0.024770784;
        // Everything except rp
        let config = vec![
            create_set32!('g', 'm', 'x'),
            create_set32!('z', 'e', 'f'),
            create_set32!('q', '\'', 'h', 'd'),
            create_set32!('l', 'j', 'i'),
            create_set32!('o', 't'),
            create_set32!('s', 'u'),
            create_set32!('a', 'k', 'w'),
            create_set32!('n', 'b'),
            create_set32!('v', 'y', 'c'),
        ];
        let (score1, _, _) = evaluate_full(&freq_list, &config, target, 10);
        assert!(score1 <= target);

        // Everything except rp, vyc
        let config = vec![
            create_set32!('g', 'm', 'x'),
            create_set32!('z', 'e', 'f'),
            create_set32!('q', '\'', 'h', 'd'),
            create_set32!('l', 'j', 'i'),
            create_set32!('o', 't'),
            create_set32!('s', 'u'),
            create_set32!('a', 'k', 'w'),
            create_set32!('n', 'b'),
        ];
        let (score2, _, _) = evaluate_full(&freq_list, &config, target, 10);
        assert!(score2 <= target);

        // Everything except rp, vyc, nb
        let config = vec![
            create_set32!('g', 'm', 'x'),
            create_set32!('z', 'e', 'f'),
            create_set32!('q', '\'', 'h', 'd'),
            create_set32!('l', 'j', 'i'),
            create_set32!('o', 't'),
            create_set32!('s', 'u'),
            create_set32!('a', 'k', 'w'),
        ];
        let (score3, _, _) = evaluate_full(&freq_list, &config, target, 10);
        assert!(score3 <= target);

        assert!(score1 >= score2);
        assert!(score2 >= score3);
    }
    #[test]
    fn evaluate_get_missing_letters() {
        let config = vec![
            create_set32!('a'),
            create_set32!('b'),
            create_set32!('c'),
            create_set32!('d', 'e', 'f'),
        ];
        let unused = get_missing_letters(&config);
        assert_eq!(unused.len(), 21);
    }
    #[test]
    fn evaluate_merge_adjacent_complex() {
        let keys = vec![create_set32!('a', 'b'), create_set32!('c', 'd')];
        let unused_letters = get_missing_letters(&keys);
        let merged_sets = merge_slices(&keys, &unused_letters);

        for s in &merged_sets {
            println!("{}", set32s_to_string(&s));
        }
        // assert_eq!(merged_sets.len(), 6);
    }
    #[test]
    fn evaluate_merge_adjacent_simple() {
        let unused_letters = vec![create_set32!('a'), create_set32!('b'), create_set32!('c')];
        let keys = vec![create_set32!('d', 'e')];
        let merged_sets = merge_slices(&keys, &unused_letters);

        for s in &merged_sets {
            println!("{}", set32s_to_string(&s));
        }
        assert_eq!(merged_sets.len(), 6);
    }

    fn str_to_set(s: &str) -> Set32 {
        let mut res = Set32::EMPTY;
        for c in s.chars() {
            res = res.add(CHAR_TO_INDEX[&c].try_into().unwrap());
        }
        res
    }
    #[test]
    fn evaluate_one_char_per_letter_0() {
        let fl = create_freq_list("./word_freq.json");
        let mut config = Vec::<Set32>::new();
        config.push(Set32::fill_from_right(1));
        config.append(&mut get_missing_letters(&config));
        let score = evaluate(&fl, &config, 10000.9);
        assert_eq!(score, 0.0);
    }
    #[test]
    fn evaluate_abc_same_as_python_result() {
        let fl = create_freq_list("./word_freq.json");
        let mut config: Vec<Set32> = vec![
            str_to_set("abc"),
            str_to_set("def"),
            str_to_set("ghi"),
            str_to_set("jkl"),
            str_to_set("mno"),
            str_to_set("pqr"),
            str_to_set("st"),
            str_to_set("uv"),
            str_to_set("wx"),
            str_to_set("yz'"),
        ];

        let mut config_hm: HashMap<u8, u8> = HashMap::new();
        config.extend(&get_missing_letters(&config));
        config_to_t10_config_full(&config, &mut config_hm);
        let score = evaluate(&fl, &config, 10000.9);
        let python_score: f32 = 0.080390571;
        let epsilon = 0.0001;
        assert!((score - python_score).abs() < epsilon);
    }
    // cargo test --release test_evaluate_arr_vs_evaluate -- --nocapture
    #[test]
    fn test_evaluate_arr_vs_evaluate() {
        // Run each evaluation function 50 times to get better performance metrics

        // Create frequency list
        let freq_list = create_freq_list("./word_freq.json");

        // Use a configuration that includes all letters
        let config = vec![
            create_set32_str!("gmx"),
            create_set32_str!("zef"),
            create_set32_str!("q'hd"),
            create_set32_str!("lji"),
            create_set32_str!("ot"),
            create_set32_str!("su"),
            create_set32_str!("akw"),
            create_set32_str!("nb"),
            create_set32_str!("vyc"),
            create_set32_str!("rp"),
        ];

        // Make sure all letters are used
        let missing = get_missing_letters(&config);
        assert!(missing.is_empty(), "Config should use all letters");

        // Time and execute evaluate
        let stop_at = f32::MAX; // No early stopping
        let iterations = 50;

        // Time and execute evaluate multiple times
        let start = Instant::now();
        let mut score1 = 0.0;
        for _ in 0..iterations {
            score1 = evaluate(&freq_list, &config, stop_at);
        }
        let duration1 = start.elapsed();
        let avg_duration1 = duration1.as_secs_f64() / iterations as f64;

        // Time and execute evaluate_arr multiple times
        let start = Instant::now();
        let mut score2 = 0.0;
        let freq_list: Vec<(Vec<u8>, f32)> = freq_list;
        let hm = &mut HashMap::new();
        for _ in 0..iterations {
            score2 = evaluate_arr(&freq_list, &config, hm, stop_at);
        }
        let duration2 = start.elapsed();
        let avg_duration2 = duration2.as_secs_f64() / iterations as f64;

        // Verify results are the same
        assert_eq!(
            score1, score2,
            "evaluate and evaluate_arr should return the same score"
        );

        println!("Running each function {} times:", iterations);
        println!(
            "evaluate:     {:.4}ms total, {:.4}ms per run",
            duration1.as_secs_f64() * 1000.0,
            avg_duration1 * 1000.0
        );
        println!(
            "evaluate_arr: {:.4}ms total, {:.4}ms per run",
            duration2.as_secs_f64() * 1000.0,
            avg_duration2 * 1000.0
        );
        println!("Speedup: {:.2}x", avg_duration1 / avg_duration2);
    }

    #[test]
    // cargo test --release test_evaluate_arr_vs_evaluate_arr_int -- --nocapture
    fn test_evaluate_arr_vs_evaluate_arr_int() {
        // Create frequency list and convert to integer version
        let freq_list = create_freq_list("./word_freq.json");
        let scale = 1_000_000_000_000.0; // Scale factor for conversion to u64
        let freq_list_int: Vec<(Vec<u8>, u64)> = freq_list
            .iter()
            .map(|(word, freq)| (word.clone(), (*freq * scale) as u64))
            .collect();

        // Use a configuration that includes all letters
        let config = vec![
            create_set32_str!("gmx"),
            create_set32_str!("zef"),
            create_set32_str!("q'hd"),
            create_set32_str!("lji"),
            create_set32_str!("ot"),
            create_set32_str!("su"),
            create_set32_str!("akw"),
            create_set32_str!("nb"),
            create_set32_str!("vyc"),
            create_set32_str!("rp"),
        ];

        // Make sure all letters are used
        let missing = get_missing_letters(&config);
        assert!(missing.is_empty(), "Config should use all letters");

        // No early stopping
        let stop_at_float = f32::MAX;
        let stop_at_int = u64::MAX;
        let iterations = 50;

        // Time and execute evaluate_arr multiple times

        let mut score_float = 0.0;
        let hm_float = &mut HashMap::new();
        let start = Instant::now();
        for _ in 0..iterations {
            hm_float.clear();
            score_float = evaluate_arr(&freq_list, &config, hm_float, stop_at_float);
        }
        let duration_float = start.elapsed();
        let avg_duration_float = duration_float.as_secs_f64() / iterations as f64;

        // Time and execute evaluate_arr_int multiple times
        let start = Instant::now();
        let mut score_int = 0;
        let hm_int = &mut HashMap::new();
        for _ in 0..iterations {
            hm_int.clear();
            score_int = evaluate_arr_int(&freq_list_int, &config, hm_int, stop_at_int);
        }
        let duration_int = start.elapsed();
        let avg_duration_int = duration_int.as_secs_f64() / iterations as f64;

        // Verify results are comparable (with some tolerance for floating point)
        let float_score_scaled = score_float * scale;
        let diff_percentage =
            ((float_score_scaled - score_int as f32).abs() / float_score_scaled) * 100.0;
        assert!(
            diff_percentage < 0.01,
            "Scores should be very close: float={}, int={}, diff={}%",
            float_score_scaled,
            score_int,
            diff_percentage
        );

        println!("Running each function {} times:", iterations);
        println!(
            "evaluate_arr (float): {:.4}ms total, {:.4}ms per run",
            duration_float.as_secs_f64() * 1000.0,
            avg_duration_float * 1000.0
        );
        println!(
            "evaluate_arr_int:     {:.4}ms total, {:.4}ms per run",
            duration_int.as_secs_f64() * 1000.0,
            avg_duration_int * 1000.0
        );
        println!("Speedup: {:.2}x", avg_duration_float / avg_duration_int);
    }
    // RUSTFLAGS="-Awarnings" cargo test --release test_word_to_t10_idx_vs_arr -- --nocapture
    #[test]
    fn test_word_to_t10_idx_vs_arr() {
        // Create a configuration array
        let config = vec![
            create_set32_str!("gmx"),
            create_set32_str!("zef"),
            create_set32_str!("q'hd"),
            create_set32_str!("lji"),
            create_set32_str!("ot"),
            create_set32_str!("su"),
            create_set32_str!("akw"),
            create_set32_str!("nb"),
            create_set32_str!("vyc"),
            create_set32_str!("rp"),
        ];

        let config_arr = config_to_10_arr(&config);

        // Create sample words of different lengths
        // Create sample words of different lengths from the frequency list
        let freq_list = create_freq_list("./word_freq.json");
        let words: Vec<Vec<u8>> = freq_list.iter().map(|(word, _)| word.clone()).collect();

        // Make sure we have words
        assert!(
            !words.is_empty(),
            "Failed to load words from frequency list"
        );

        let iterations = 100;

        // Test word_to_t10_arr
        let start = Instant::now();
        let mut _result1 = Vec::with_capacity(iterations * words.len());
        for _ in 0..iterations {
            for word in &words {
                _result1.push(word_to_t10_arr(word, &config_arr)[0]);
            }
        }
        let duration_arr = start.elapsed();

        // Test word_to_t10_idx
        let start = Instant::now();
        let mut _result2: Vec<u64> = Vec::with_capacity(iterations * words.len());
        for _ in 0..iterations {
            for word in &words {
                _result2.push(word_to_t10_idx(word, &config_arr));
            }
        }
        let duration_idx = start.elapsed();

        println!(
            "Running each function {} times on {} words:",
            iterations,
            words.len()
        );
        println!(
            "word_to_t10_arr: {:.4}ms total, {:.9}ms per word",
            duration_arr.as_secs_f64() * 1000.0,
            duration_arr.as_secs_f64() * 1000.0 / (iterations * words.len()) as f64
        );
        println!(
            "word_to_t10_idx: {:.4}ms total, {:.9}ms per word",
            duration_idx.as_secs_f64() * 1000.0,
            duration_idx.as_secs_f64() * 1000.0 / (iterations * words.len()) as f64
        );
        println!(
            "Ratio arr/idx: {:.2}x",
            duration_arr.as_secs_f64() / duration_idx.as_secs_f64()
        );
    }

    // cargo test --release test_evaluate_arr_vs_evaluate_arr_idx -- --nocapture
    #[test]
    fn test_evaluate_arr_vs_evaluate_arr_idx() {
        // Create frequency list
        let freq_list = create_freq_list("./word_freq.json");

        // Use a configuration that includes all letters
        let config = vec![
            create_set32_str!("gmx"),
            create_set32_str!("zef"),
            create_set32_str!("q'hd"),
            create_set32_str!("lji"),
            create_set32_str!("ot"),
            create_set32_str!("su"),
            create_set32_str!("akw"),
            create_set32_str!("nb"),
            create_set32_str!("vyc"),
            create_set32_str!("rp"),
        ];

        // Make sure all letters are used
        let missing = get_missing_letters(&config);
        assert!(missing.is_empty(), "Config should use all letters");

        // No early stopping
        let stop_at = f32::MAX;
        let iterations = 20;

        // Time and execute evaluate_arr multiple times
        let start = Instant::now();
        let mut score_arr = 0.0;
        let hm_arr = &mut HashMap::<Vec<u8>, u32>::new();
        for _ in 0..iterations {
            hm_arr.clear();
            score_arr = evaluate_arr(&freq_list, &config, hm_arr, stop_at);
        }
        let duration_arr = start.elapsed();
        let avg_duration_arr = duration_arr.as_secs_f64() / iterations as f64;

        // Time and execute evaluate_arr_idx multiple times
        let start = Instant::now();
        let mut score_idx = 0.0;
        let hm_idx = &mut HashMap::<u128, u32>::new();
        for _ in 0..iterations {
            hm_idx.clear();
            score_idx = evaluate_arr_idx(&freq_list, &config, hm_idx, stop_at);
        }
        let duration_idx = start.elapsed();
        let avg_duration_idx = duration_idx.as_secs_f64() / iterations as f64;

        // Verify results are the same (with small tolerance for floating point)
        let epsilon = 0.0001;
        assert!(
            (score_arr - score_idx).abs() < epsilon,
            "Scores should be equal: arr={}, idx={}, diff={}",
            score_arr,
            score_idx,
            (score_arr - score_idx).abs()
        );

        println!("Running each function {} times:", iterations);
        println!(
            "evaluate_arr:     {:.4}ms total, {:.4}ms per run",
            duration_arr.as_secs_f64() * 1000.0,
            avg_duration_arr * 1000.0
        );
        println!(
            "evaluate_arr_idx: {:.4}ms total, {:.4}ms per run",
            duration_idx.as_secs_f64() * 1000.0,
            avg_duration_idx * 1000.0
        );
        println!("Speedup: {:.2}x", avg_duration_arr / avg_duration_idx);
    }
    // RUSTFLAGS="-Awarnings" cargo test --release test_vec_vs_flat_array_iteration -- --nocapture
    #[test]
    fn test_vec_vs_flat_array_iteration() {
        // Create a Vec<Vec<u8>> with 10,000 vectors, each containing 5 u8s

        let mut nested_vec: Vec<Vec<u8>> = Vec::with_capacity(10_000);
        for i in 0..10_000 {
            let values = vec![
                (i % 256) as u8,
                ((i + 1) % 256) as u8,
                ((i + 2) % 256) as u8,
                ((i + 3) % 256) as u8,
                ((i + 4) % 256) as u8,
            ];
            nested_vec.push(values);
        }

        // Create a flat Vec<u8> with 50,000 elements (equivalent data size)
        let mut flat_vec: Vec<u8> = Vec::with_capacity(50_000);
        for i in 0..50_000 {
            flat_vec.push((i % 256) as u8);
        }

        // Number of iterations for more accurate timing
        let iterations = 1000;

        // Test Vec<Vec<u8>> iteration
        let mut ap = vec![];
        let mut sum_nested = 0u64;
        let start = Instant::now();
        for _ in 0..iterations {
            for inner_vec in &nested_vec {
                for &value in inner_vec {
                    sum_nested += value as u64;
                    ap.push(value);
                }
            }
        }
        let nested_duration = start.elapsed();

        // Test flat Vec<u8> iteration
        // ap.clear();
        let start = Instant::now();
        for _ in 0..iterations {
            for &value in &flat_vec {
                sum_nested += value as u64;
                ap.push(value);
            }
        }
        let flat_duration = start.elapsed();
        assert!(
            sum_nested > 0,
            "Sum should be greater than 0 to prevent optimization"
        );
        // Calculate the element access count for fair comparison
        let nested_accesses = iterations * 10_000 * 5;
        let flat_accesses = iterations * 50_000;
        assert_eq!(
            nested_accesses, flat_accesses,
            "Access counts should be equal"
        );

        // Report results
        println!("Iteration performance comparison:");
        println!(
            "Vec<Vec<u8>> ({} elements): {:.3}ms total, {:.9}ms per element",
            nested_vec.len() * 5,
            nested_duration.as_secs_f64() * 1000.0,
            nested_duration.as_secs_f64() * 1000.0 / nested_accesses as f64
        );

        println!(
            "Flat Vec<u8> ({} elements): {:.3}ms total, {:.9}ms per element",
            flat_vec.len(),
            flat_duration.as_secs_f64() * 1000.0,
            flat_duration.as_secs_f64() * 1000.0 / flat_accesses as f64
        );

        println!(
            "Speedup (flat vs nested): {:.2}x",
            nested_duration.as_secs_f64() / flat_duration.as_secs_f64()
        );

        // Verify that sum calculation was correct (prevent compiler optimization)
        // println!("Sums (to prevent optimization): nested={}, flat={}", sum_nested, sum_flat);
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_evaluate_arr_opt_vs_evaluate_arr_idx -- --nocapture
    #[test]
    fn test_evaluate_arr_opt_vs_evaluate_arr_idx() {
        // Create frequency list
        let freq_list = create_freq_list("./word_freq.json");

        // Group the frequency list by word length
        let freq_lists_opt =
            crate::general::freq_list::group_by_length_flattened(freq_list.clone());

        // Use a configuration that includes all letters
        let config = vec![
            create_set32_str!("gmx"),
            create_set32_str!("zef"),
            create_set32_str!("q'hd"),
            create_set32_str!("lji"),
            create_set32_str!("ot"),
            create_set32_str!("su"),
            create_set32_str!("akw"),
            create_set32_str!("nb"),
            create_set32_str!("vyc"),
            create_set32_str!("rp"),
        ];

        // Make sure all letters are used
        let missing = get_missing_letters(&config);
        assert!(missing.is_empty(), "Config should use all letters");

        // No early stopping
        let stop_at = f32::MAX;
        let iterations = 10;

        // Time and execute standard evaluate
        let start = Instant::now();
        let mut score_standard = 0.0;
        for _ in 0..iterations {
            score_standard = evaluate(&freq_list, &config, stop_at);
        }
        let duration_standard = start.elapsed();
        let avg_duration_standard = duration_standard.as_secs_f64() / iterations as f64;

        // Time and execute evaluate_arr_idx multiple times
        let start = Instant::now();
        let mut score_idx = 0.0;
        let hm_idx = &mut HashMap::<u128, u32>::new();
        for _ in 0..iterations {
            hm_idx.clear();
            score_idx = evaluate_arr_idx(&freq_list, &config, hm_idx, stop_at);
        }
        let duration_idx = start.elapsed();
        let avg_duration_idx = duration_idx.as_secs_f64() / iterations as f64;

        let mut count_hm_64: FxHashMap<u64, u8> = FxHashMap::<u64, u8>::new();
        let mut count_hm_128: FxHashMap<u128, u8> = FxHashMap::<u128, u8>::new();
        let mut count_hm_vec: FxHashMap<Vec<u8>, u8> = FxHashMap::<Vec<u8>, u8>::new();

        // Time and execute evaluate_arr_opt multiple times
        let start = Instant::now();
        let mut score_opt = 0.0;
        for _ in 1..iterations + 1 {
            (score_opt, _) = evaluate_arr_opt(
                &freq_lists_opt,
                &config,
                &mut count_hm_64,
                &mut count_hm_128,
                &mut count_hm_vec,
                stop_at,
            );
            count_hm_128.clear();
            count_hm_64.clear();
            count_hm_vec.clear();
        }
        let duration_opt = start.elapsed();
        let avg_duration_opt = duration_opt.as_secs_f64() / iterations as f64;

        // Print results
        println!("Running each function {} times:", iterations);
        println!(
            "standard evaluate: {:.4}ms total, {:.4}ms per run",
            duration_standard.as_secs_f64() * 1000.0,
            avg_duration_standard * 1000.0
        );
        println!(
            "evaluate_arr_idx:  {:.4}ms total, {:.4}ms per run",
            duration_idx.as_secs_f64() * 1000.0,
            avg_duration_idx * 1000.0
        );
        println!(
            "evaluate_arr_opt:  {:.4}ms total, {:.4}ms per run",
            duration_opt.as_secs_f64() * 1000.0,
            avg_duration_opt * 1000.0
        );

        // Print speedups relative to standard evaluate
        println!("Speedups relative to standard evaluate:");
        println!(
            "evaluate_arr_idx: {:.2}x",
            avg_duration_standard / avg_duration_idx
        );
        println!(
            "evaluate_arr_opt: {:.2}x",
            avg_duration_standard / avg_duration_opt
        );

        // If opt is faster than idx, show that speedup too
        if avg_duration_opt < avg_duration_idx {
            println!(
                "Speedup opt vs idx: {:.2}x",
                avg_duration_idx / avg_duration_opt
            );
        } else {
            println!(
                "Slowdown opt vs idx: {:.2}x",
                avg_duration_opt / avg_duration_idx
            );
        }

        // Note: The scores might not match exactly due to different data structures
        // and processing methods, but they should be in the same ballpark
        println!(
            "Scores: standard={}, idx={}, opt={}",
            score_standard, score_idx, score_opt
        );
    }
    #[test]
    fn test_freq_list_clone_time() {
        // Load the frequency list
        let freq_list = create_freq_list("./word_freq.json");

        // Measure time to clone the frequency list
        let iterations = 100;
        let start = Instant::now();
        let mut clones = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            clones.push(freq_list.clone());
        }

        let duration = start.elapsed();
        let avg_duration = duration.as_secs_f64() / iterations as f64;

        // Force compiler to keep the clones
        assert!(!clones.is_empty());

        println!("Frequency list clone performance:");
        println!("List size: {} entries", freq_list.len());
        println!(
            "Total time for {} clones: {:.4}ms",
            iterations,
            duration.as_secs_f64() * 1000.0
        );
        println!("Average time per clone: {:.4}ms", avg_duration * 1000.0);
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_keyboard_score_20000 -- --nocapture
    #[test]
    fn test_keyboard_score_20000() {
        // Create frequency list
        let freq_list = create_freq_list("./word_freq_20000.json");

        // Create config for the specified keyboard layout
        // Create config for the specified keyboard layout
        let config = vec![
            create_set32_str!("akw"),  // d
            create_set32_str!("bn"),   // h
            create_set32_str!("cez"),  // g
            create_set32_str!("df"),   // c
            create_set32_str!("gmx"),  // a
            create_set32_str!("hiv"), // i{'b': 'akw', 'g': 'bn', 'e': 'cez', 'd': 'df', 'a': 'gmx', 'i': 'hiv', 'f': 'jlqy', 'j': 'ot', 'c': "pr'", 'h': 'su'}
            create_set32_str!("jlqy"), // e
            create_set32_str!("ot"),  // j
            create_set32_str!("pr'"), // b
            create_set32_str!("su"),  // f
        ];

        // Make sure all letters are accounted for
        let missing = get_missing_letters(&config);
        assert!(
            missing.is_empty(),
            "Keyboard should use all letters, missing: {:?}",
            missing
        );

        // Group the frequency list by word length for optimized evaluation
        let freq_lists_opt =
            crate::general::freq_list::group_by_length_flattened(freq_list.clone());

        let mut count_hm_64: FxHashMap<u64, u8> = FxHashMap::new();
        let mut count_hm_128: FxHashMap<u128, u8> = FxHashMap::new();
        let mut count_hm_vec: FxHashMap<Vec<u8>, u8> = FxHashMap::new();

        // Evaluate the keyboard
        let (score, num_keys) = evaluate_arr_opt(
            &freq_lists_opt,
            &config,
            &mut count_hm_64,
            &mut count_hm_128,
            &mut count_hm_vec,
            f32::MAX,
        );

        // Print results
        println!("Keyboard layout:");
        println!("Score: {}", score);
        println!("Number of keys: {}", num_keys);

        // Compare with standard evaluation for verification
        let standard_score = evaluate(&freq_list, &config, f32::MAX);
        println!("Standard evaluation score: {}", standard_score);

        // Verify scores are close
        let epsilon = 0.001;
        assert!(
            (score - standard_score).abs() < epsilon,
            "Scores should be close: opt={}, standard={}, diff={}",
            score,
            standard_score,
            (score - standard_score).abs()
        );
    }
    // RUSTFLAGS="-Awarnings" cargo test --release test_keyboard_score -- --nocapture
    #[test]
    fn test_keyboard_score() {
        // Create frequency list
        let freq_list = create_freq_list("./word_freq.json");

        // Create config for the specified keyboard layout
        // Create config for the specified keyboard layout
        let config = vec![
            create_set32_str!("akw"),  // d
            create_set32_str!("bn"),   // h
            create_set32_str!("cvy"),  // g
            create_set32_str!("dhq'"), // c
            create_set32_str!("efz"),  // a
            create_set32_str!("gmx"),  // i
            create_set32_str!("ijl"),  // e
            create_set32_str!("ot"),   // j
            create_set32_str!("pr"),   // b
            create_set32_str!("su"),   // f
        ];

        // Make sure all letters are accounted for
        let missing = get_missing_letters(&config);
        assert!(
            missing.is_empty(),
            "Keyboard should use all letters, missing: {:?}",
            missing
        );

        // Group the frequency list by word length for optimized evaluation
        let freq_lists_opt =
            crate::general::freq_list::group_by_length_flattened(freq_list.clone());

        let mut count_hm_64: FxHashMap<u64, u8> = FxHashMap::new();
        let mut count_hm_128: FxHashMap<u128, u8> = FxHashMap::new();
        let mut count_hm_vec: FxHashMap<Vec<u8>, u8> = FxHashMap::new();

        // Evaluate the keyboard
        let (score, num_keys) = evaluate_arr_opt(
            &freq_lists_opt,
            &config,
            &mut count_hm_64,
            &mut count_hm_128,
            &mut count_hm_vec,
            f32::MAX,
        );

        // Print results
        println!("Keyboard layout:");
        println!("i: 'am', a: 'bn', e: \"cvy'\", g: 'dfq', b: 'ewz'");
        println!("f: 'gp', j: 'hlx', d: 'ikr', h: 'jot', c: 'su'");
        println!("Score: {}", score);
        println!("Number of keys: {}", num_keys);

        // Compare with standard evaluation for verification
        let standard_score = evaluate(&freq_list, &config, f32::MAX);
        println!("Standard evaluation score: {}", standard_score);

        // Verify scores are close
        let epsilon = 0.001;
        assert!(
            (score - standard_score).abs() < epsilon,
            "Scores should be close: opt={}, standard={}, diff={}",
            score,
            standard_score,
            (score - standard_score).abs()
        );
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_keyboard_score_20 -- --nocapture
    #[test]
    fn test_keyboard_score_20() {
        // Create frequency list
        let freq_list = create_freq_list("./word_freq_20000.json");

        // Create config for the specified keyboard layout

        let config = vec![
            create_set32_str!("akw"),  // b
            create_set32_str!("bn"),   // g
            create_set32_str!("cez"),  // e
            create_set32_str!("df"),   // d
            create_set32_str!("gmx"),  // a
            create_set32_str!("hiv"),  // i
            create_set32_str!("jlqy"), // f
            create_set32_str!("ot"),   // j
            create_set32_str!("pr'"),  // c
            create_set32_str!("su"),   // h
        ];

        // Make sure all letters are accounted for
        let missing = get_missing_letters(&config);
        assert!(
            missing.is_empty(),
            "Keyboard should use all letters, missing: {:?}",
            missing
        );

        // Group the frequency list by word length for optimized evaluation
        let freq_lists_opt =
            crate::general::freq_list::group_by_length_flattened(freq_list.clone());

        let mut count_hm_64: FxHashMap<u64, u8> = FxHashMap::new();
        let mut count_hm_128: FxHashMap<u128, u8> = FxHashMap::new();
        let mut count_hm_vec: FxHashMap<Vec<u8>, u8> = FxHashMap::new();

        // Evaluate the keyboard
        let (score, num_keys) = evaluate_arr_opt(
            &freq_lists_opt,
            &config,
            &mut count_hm_64,
            &mut count_hm_128,
            &mut count_hm_vec,
            f32::MAX,
        );

        // Print results
        println!("Keyboard layout:");
        println!("i: 'am', a: 'bn', e: \"cvy'\", g: 'dfq', b: 'ewz'");
        println!("f: 'gp', j: 'hlx', d: 'ikr', h: 'jot', c: 'su'");
        println!("Score: {}", score);
        println!("Number of keys: {}", num_keys);

        // Compare with standard evaluation for verification
        let standard_score = evaluate(&freq_list, &config, f32::MAX);
        println!("Standard evaluation score: {}", standard_score);

        // Verify scores are close
        let epsilon = 0.001;
        assert!(
            (score - standard_score).abs() < epsilon,
            "Scores should be close: opt={}, standard={}, diff={}",
            score,
            standard_score,
            (score - standard_score).abs()
        );
    }
}

/*
1: 3
2: 4
3: 6
*/
