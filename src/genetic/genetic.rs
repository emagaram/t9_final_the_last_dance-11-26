use std::collections::hash_map::Entry;
use std::fs::{self, File};
use std::io::{self, Write};

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use chrono::{DateTime, Local};
use fxhash::FxHashMap;

use gxhash::{HashMap, HashMapExt};
use rand::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::general::freq_list::{
    create_freq_list32, group_by_length_flattened, FreqListsFlattened,
};
use crate::globals::{INDEX_TO_CHAR, NUM_LETTERS};
use crate::settings::NUM_KEYS;

// Config type - maps letters to keys
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Config {
    letter_to_keys: [u128; NUM_LETTERS],
    key_count: [usize; NUM_KEYS],
}

impl Config {
    /// Generates a random T10 configuration
    fn generate_random_t10_config(rng: &mut ThreadRng, letter_arr: &mut [usize]) -> Config {
        let mut config: Config = Config {
            letter_to_keys: [0; NUM_LETTERS],
            key_count: [0; NUM_KEYS],
        };
        let mut key_id = 0;
        letter_arr.shuffle(rng);
        for i in 0..NUM_LETTERS {
            config.letter_to_keys[letter_arr[i]] = key_id;
            config.key_count[key_id as usize] += 1;
            key_id += 1;
            key_id %= NUM_KEYS as u128;
        }
        assert!(config.key_count.iter().fold(0, |acc, &x| acc + x) == NUM_LETTERS);
        config
    }

    fn calculate_hash_key(&self) -> u128 {
        let base: u128 = NUM_KEYS as u128 + 1;
        self.letter_to_keys
            .iter()
            .fold(0, |acc, &key| acc * base + (key + 1))
    }
}

/// Monitors for termination signal in a separate thread
pub fn monitor_for_termination(stop_flag: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut input = String::new();
        loop {
            input.clear();
            match io::stdin().read_line(&mut input) {
                Ok(_) => {
                    if input.trim().to_lowercase() == "end" {
                        stop_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from stdin: {}", e);
                    break;
                }
            }
        }
    });
}

/// Writes the current end time to a file
fn write_end_time(base_path: &str) -> io::Result<()> {
    let now: DateTime<Local> = Local::now();
    let end_time = now.format("%Y_%m_%d_%Hh_%Mm_%Ss").to_string();
    let filename = Path::new(base_path).join("end_time.txt");

    fs::write(filename, end_time)?;
    Ok(())
}

/// Reads the best score from a file
fn read_best_score_from_file(base_path: &str, score_file: &str) -> (f32, Option<Config>) {
    // Initialize default values
    let mut best_score = f32::MAX;
    let mut best_config = None;

    let path_name = Path::new(base_path).join("solutions").join(score_file);

    if path_name.exists() {
        match fs::read_to_string(&path_name) {
            Ok(contents) => {
                let lines: Vec<&str> = contents.lines().collect();
                if lines.len() >= 2 {
                    if let Ok(score) = lines[0].parse::<f32>() {
                        best_score = score;
                        // Parse the config from the second line
                        if let Ok(config) = serde_json::from_str::<Config>(lines[1]) {
                            best_config = Some(config);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading file {}: {}", path_name.display(), e);
            }
        }
    }

    (best_score, best_config)
}

/// Writes the best score to a file
fn write_best_score_to_file(
    base_path: &str,
    score: f32,
    config: &Config,
    score_file: &str,
) -> io::Result<()> {
    // Ensure the Solutions folder exists
    let path_name = Path::new(base_path).join("solutions");
    fs::create_dir_all(&path_name)?;

    // Path to the file within the Solutions folder
    let full_path = path_name.join(score_file);

    // Write the score and config to the file
    let mut file = File::create(full_path)?;
    writeln!(file, "{}", score)?;
    writeln!(
        file,
        "{}",
        serde_json::to_string(&reverse_dictionary(config))?
    )?;

    Ok(())
}

/// Calculates the deadline for solution improvement
fn solution_improvement_deadline(
    solution_improvement_deadline_base: u32,
    solution_improvement_deadline_exp: u32,
    all_time_score: f32,
    current_best_score: f32,
) -> u32 {
    let m;
    if 1.0 < current_best_score / all_time_score {
        m = 1.0;
    } else {
        m = current_best_score / all_time_score;
    }
    let closeness_boost = 3
        * solution_improvement_deadline_base
        * m.powf(solution_improvement_deadline_exp as f32) as u32;

    return solution_improvement_deadline_base + closeness_boost;
}

/// Reverses the mapping of a dictionary
fn reverse_dictionary(config: &Config) -> HashMap<u128, String> {
    let mut reverse_dict: HashMap<u128, String> = HashMap::new();

    for (letter, key) in config.letter_to_keys.iter().enumerate() {
        let letter_char = &INDEX_TO_CHAR[&(letter as u8)];
        reverse_dict
            .entry(*key)
            .and_modify(|v| *v += letter_char)
            .or_insert_with(|| letter_char.clone());
    }

    reverse_dict
}

fn process_hash_entry<K>(entry: Entry<K, u8>, score: &mut f32, word_score: f32) {
    match entry {
        Entry::Occupied(mut e) => {
            let val = e.get_mut();
            *score += word_score * *val as f32;
            *val += 1;
        }
        Entry::Vacant(e) => {
            e.insert(1);
        }
    }
}

fn evaluate_t10_config(
    freq_lists: &[(u8, Vec<u8>)],
    config: &Config,
    count_hm_64: &mut FxHashMap<u64, u8>,
    count_hm_128: &mut FxHashMap<u128, u8>,
    count_hm_vec: &mut FxHashMap<Vec<u8>, u8>,
    stop_at: f32,
) -> f32 {
    let mut score: f32 = 0.0;

    // Calculate maximum word length that can be represented in each type
    // let max_idx_len = count_arr.len().ilog(num_keys + 1) as u8;
    let max_u64_len = u64::MAX.ilog((NUM_KEYS + 1) as u64) as u8;
    let max_u128_len = u128::MAX.ilog((NUM_KEYS + 1) as u128) as u8;
    let conf_size = NUM_KEYS as u64;
    let mut config = config.clone();
    for c in config.letter_to_keys.iter_mut() {
        *c += 1;
    }
    // for (i, c) in config.letter_to_keys.iter().enumerate() {
    //     println!("config_arr[{}]: {}", i, c);
    // }
    count_hm_64.clear();
    count_hm_128.clear();
    count_hm_vec.clear();
    for (len, freq_list) in freq_lists {
        let len_size = *len as usize;
        let inc = len_size + 4;

        for chunk in freq_list.chunks_exact(inc) {
            // Get word score using unsafe pointer arithmetic
            let score_bytes = unsafe { chunk.get_unchecked(len_size..len_size + 4) };
            let word_score = f32::from_le_bytes(score_bytes.try_into().unwrap());
            let word_bytes = unsafe { chunk.get_unchecked(..len_size) };

            if *len <= max_u64_len {
                let mut word_idx: u64 = 0;
                for &byte in word_bytes {
                    word_idx = word_idx * conf_size
                        + unsafe { (*config.letter_to_keys.get_unchecked(byte as usize)) as u64 };
                }
                process_hash_entry(count_hm_64.entry(word_idx), &mut score, word_score)
            } else if *len <= max_u128_len {
                let mut word_idx: u128 = 0;
                for &byte in word_bytes {
                    word_idx = word_idx * conf_size as u128
                        + unsafe { (*config.letter_to_keys.get_unchecked(byte as usize)) as u128 };
                }
                process_hash_entry(count_hm_128.entry(word_idx as u128), &mut score, word_score)
            } else {
                // Vec<u8> implementation for very long words
                let mut word_vec = Vec::with_capacity(len_size);
                for &byte in word_bytes {
                    let value =
                        unsafe { *config.letter_to_keys.get_unchecked(byte as usize) as u8 };
                    word_vec.push(value);
                }
                process_hash_entry(count_hm_vec.entry(word_vec), &mut score, word_score)
            }
            if score > stop_at {
                return score as f32;
            }
        }
    }
    return score as f32;
}

/// Mutates a configuration
fn mutate(config: &mut Config, rng: &mut ThreadRng) {
    let indices: Vec<usize> = (0..NUM_LETTERS).sample(rng, 2);
    let letter1 = indices[0];
    let letter2 = indices[1];
    let key1 = config.letter_to_keys[letter1];
    let key2 = config.letter_to_keys[letter2];

    config.letter_to_keys[letter1] = key2;
    config.letter_to_keys[letter2] = key1;
}

/// Writes the number of completed generations to a file
fn write_generations_completed(
    base_path: &str,
    process_id: usize,
    generations: u32,
) -> io::Result<()> {
    let directory_path = Path::new(base_path).join("performance");
    fs::create_dir_all(&directory_path)?;

    let filename = directory_path.join(format!("{}_generations_completed.txt", process_id));
    fs::write(filename, generations.to_string())?;

    Ok(())
}

fn evaluate_population_member(
    config: &Config,
    scores_cache: &mut HashMap<u128, f32>,
    freq_lists: &[(u8, Vec<u8>)],
    all_time_score: f32,
    count_hm_64: &mut FxHashMap<u64, u8>,
    count_hm_128: &mut FxHashMap<u128, u8>,
    count_hm_vec: &mut FxHashMap<Vec<u8>, u8>,
) -> f32 {
    let config_hash = config.calculate_hash_key();
    if let Some(&score) = scores_cache.get(&config_hash) {
        return score;
    }

    let score = evaluate_t10_config(
        freq_lists,
        config,
        count_hm_64,
        count_hm_128,
        count_hm_vec,
        all_time_score,
    );
    // println!("Config evaluated in {:?}, score: {}", elapsed, score);
    scores_cache.insert(config_hash, score);
    score
}

/// Main simulation function
pub fn run_simulation(
    iteration_path: &str,
    freq_lists: &FreqListsFlattened,
    stop_flag: Arc<AtomicBool>,
    process_id: usize,
) -> io::Result<()> {
    let num_keys = 10;
    let population_size = 60;
    let solution_improvement_deadline_base = 3000;
    let solution_improvement_deadline_exp = 5;
    let score_file = format!("{}_best_{}_key_layout.txt", process_id, num_keys);
    let mut current_best_score = f32::MAX;

    let mut rng: ThreadRng = rand::rng();
    // Create an array with indices from 0 to NUM_LETTERS-1
    let mut letters_arr: [usize; NUM_LETTERS] = [0; NUM_LETTERS];
    for i in 0..NUM_LETTERS {
        letters_arr[i] = i;
    }

    // Read existing best score if available
    let (mut all_time_score, _) = read_best_score_from_file(iteration_path, &score_file);

    // Setup error logging
    let errors_path = Path::new(iteration_path).join("errors");
    fs::create_dir_all(&errors_path)?;

    // Initialize population
    let mut population: Vec<Config> = (0..population_size)
        .map(|_| Config::generate_random_t10_config(&mut rng, &mut letters_arr))
        .collect();

    let mut scores_cache = HashMap::new();
    let mut generation_count = 1;
    let mut total_generation_count = 0;
    let mut solution_improvement_count = 0;

    let mut count_hm_64: FxHashMap<u64, u8> = FxHashMap::default();
    let mut count_hm_128: FxHashMap<u128, u8> = FxHashMap::default();
    let mut count_hm_vec: FxHashMap<Vec<u8>, u8> = FxHashMap::default();

    while !stop_flag.load(Ordering::SeqCst) {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        // Check if we need to reset when no improvements have been made
        if solution_improvement_count
            > solution_improvement_deadline(
                solution_improvement_deadline_base,
                solution_improvement_deadline_exp,
                all_time_score,
                current_best_score,
            )
        {
            total_generation_count += generation_count;
            write_generations_completed(iteration_path, process_id, total_generation_count)?;

            println!(
                "Generation died. Total generations: {},  Reg Score: {}",
                generation_count, current_best_score
            );

            population = (0..population_size)
                .map(|_| Config::generate_random_t10_config(&mut rng, &mut letters_arr))
                .collect();

            generation_count = 1;
            solution_improvement_count = 0;
            current_best_score = f32::MAX;
            continue;
        }

        // Evaluate population in parallel
        let evaluation_results: Vec<(f32, Config)> = population
            .into_iter()
            .map(|config| {
                let score = evaluate_population_member(
                    &config,
                    &mut scores_cache,
                    &freq_lists,
                    current_best_score,
                    &mut count_hm_64,
                    &mut count_hm_128,
                    &mut count_hm_vec,
                );
                (score, config)
            })
            .collect();

        let mut scored_population = evaluation_results;

        // Limit cache size
        if scores_cache.len() > 1_000_000 {
            println!("Clearing scores cache...");
            scores_cache.clear();
        }

        // Sort population by score (lower is better)
        scored_population.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert!(scored_population[0].0 <= scored_population[1].0);
        generation_count += 1;
        solution_improvement_count += 1;

        // Update best scores
        if scored_population[0].0 < current_best_score {
            current_best_score = scored_population[0].0;
            solution_improvement_count = 0;

            // Check if this is the best overall score
            if current_best_score < all_time_score {
                all_time_score = current_best_score;

                write_best_score_to_file(
                    iteration_path,
                    current_best_score,
                    &scored_population[0].1,
                    &score_file,
                )?;

                println!("New all-time best score: {}", all_time_score);
            }
        }

        // Selection: Top 1/5 automatically admitted, then new are generated from all
        let elite_count = scored_population.len() / 5;
        let mut next_population: Vec<Config> = scored_population
            .iter()
            .take(elite_count)
            .map(|(_, config)| config.clone())
            .collect();

        let mut potential_parents: Vec<&Config> =
            scored_population.iter().map(|(_, config)| config).collect();

        potential_parents.shuffle(&mut rng);

        // Generate new population members via crossover and mutation
        while next_population.len() < population_size {
            let parent1 = potential_parents.pop().unwrap();
            let mut child = parent1.clone();

            for _ in 0..rng.random_range(1..=10) {
                mutate(&mut child, &mut rng);
            }

            next_population.push(child);
        }

        // If we don't have enough children, add random individuals
        while next_population.len() < population_size {
            next_population.push(Config::generate_random_t10_config(
                &mut rng,
                &mut letters_arr,
            ));
        }

        population = next_population;
    }

    Ok(())
}

/// Main function to run the genetic algorithm
pub fn run_genetic_algorithm() -> io::Result<()> {
    // Setup stop flag for termination
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);
    // Start the termination monitor
    monitor_for_termination(stop_flag_clone);

    let now: DateTime<Local> = Local::now();
    let iteration_id = now.format("%Y_%m_%d_%Hh_%Mm_%Ss").to_string();

    // Create directory for output
    let base_path = "normal_genetic";
    let iteration_path = Path::new(base_path).join(&iteration_id);
    fs::create_dir_all(&iteration_path)?;

    let iteration_path_str = iteration_path.to_string_lossy().to_string();

    // Run the simulation
    let freq_lists = group_by_length_flattened(create_freq_list32("word_freq_20000.json"));

    (0..4).into_par_iter().enumerate().for_each(|(i, _)| {
        if let Err(e) = run_simulation(&iteration_path_str, &freq_lists, Arc::clone(&stop_flag), i)
        {
            eprintln!("Error in simulation: {}", e);
        }
    });

    // Write end time
    write_end_time(&iteration_path_str)?;
    println!(
        "End time written to {}",
        iteration_path.join("end_time.txt").display()
    );

    Ok(())
}
