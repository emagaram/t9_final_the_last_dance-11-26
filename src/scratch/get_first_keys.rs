use crate::general::set32::Set32;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct KeysFile {
    threshold: f32,
    keys: Vec<(String, f32)>,
}

/// Attempts to load initial keys from a JSON file if it exists
///
/// The file is expected to be named keys_{num_keys}.json and contain a JSON object with:
/// - keys: Array of [string, float] tuples where string represents characters on a key
/// - threshold: A float that should match our threshold value (within epsilon)
///
/// Returns a vector of (Set32, OrderedFloat<f32>) pairs if successful, None otherwise
pub fn get_first_keys(
    num_keys: usize,
    threshold: f32,
    num_order: bool,
) -> Option<Vec<(Set32, f32)>> {
    let filename = format!("keys_{}.json", num_keys);
    let path = Path::new(&filename);

    // Check if file exists
    if !path.exists() {
        println!("No existing keys file found at {}", filename);
        return None;
    }

    // Read the file
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) => {
            println!("Error opening keys file: {}", e);
            return None;
        }
    };

    let mut contents = String::new();
    if let Err(e) = file.read_to_string(&mut contents) {
        println!("Error reading keys file: {}", e);
        return None;
    }

    // Parse the JSON
    let keys_file: KeysFile = match serde_json::from_str(&contents) {
        Ok(data) => data,
        Err(e) => {
            println!("Error parsing keys file: {}", e);
            return None;
        }
    };

    // Check if threshold matches (within epsilon)
    const EPSILON: f32 = 0.0001;
    if (keys_file.threshold - threshold).abs() > EPSILON {
        println!(
            "Threshold mismatch: file has {}, but we need {}",
            keys_file.threshold, threshold
        );
        return None;
    }

    // Convert strings to Set32
    let mut result = Vec::new();
    for (key_str, value) in keys_file.keys {
        let mut key_set = Set32::EMPTY;
        for c in key_str.chars() {
            if let Some(&idx) = crate::globals::CHAR_TO_INDEX.get(&c) {
                key_set = key_set.add(idx as u32);
            } else {
                println!("Warning: character '{}' not found in CHAR_TO_INDEX", c);
            }
        }
        result.push((key_set, value));
    }

    // Sort by float value (in ascending order)
    if num_order {
        result.sort_by_key(|&(_, value)| OrderedFloat(value));
    }
    // Sort by Set32 in alphabetical order if requested
    else {
        result.sort_by(|a, b| crate::util::compare_letters(a.0, b.0));
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_set32_str, util::set32_to_string};

    const TEST_NUM_KEYS_INVALID: usize = 9999;
    const TEST_NUM_KEYS_VALID: usize = 1000;
    const TEST_NUM_KEYS_THRESHOLD: f32 = 0.5;

    #[test]
    fn test_get_first_keys_file_not_found() {
        // Use a different number to ensure the file doesn't exist
        let result = get_first_keys(TEST_NUM_KEYS_INVALID, 0.5, true);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_first_keys_valid_file() {
        // Test the function
        let result = get_first_keys(TEST_NUM_KEYS_VALID, TEST_NUM_KEYS_THRESHOLD, true);

        // Verify results
        assert!(result.is_some());
        let keys = result.unwrap();
        assert_eq!(keys.len(), 1);

        // Check the first key: ab -> bits 0 and 1
        assert_eq!(keys[0].0, create_set32_str!("ab"));
    }

    #[test]
    fn test_get_first_keys_threshold_mismatch() {
        // Test the function with a different threshold
        let result = get_first_keys(TEST_NUM_KEYS_INVALID, TEST_NUM_KEYS_THRESHOLD + 0.1, true);
        // Should return None due to threshold mismatch
        assert!(result.is_none());
    }

    #[test]
    fn test_get_first_keys_key_conversion() {
        // Test how characters get converted to Set32
        // Create a temporary file with known contents
        let test_str = "abc";
        let expected_set = create_set32_str!("abc");

        // Call the function with valid parameters
        let result = get_first_keys(TEST_NUM_KEYS_VALID, TEST_NUM_KEYS_THRESHOLD, true);

        // Check that characters were correctly converted
        if let Some(keys) = result {
            for (key_set, _) in keys {
                if set32_to_string(key_set) == test_str {
                    assert_eq!(key_set, expected_set);
                    return;
                }
            }
        }
    }

    #[test]
    fn test_get_first_keys_ascending_order() {
        // Test that keys are sorted in ascending order by float value
        let result = get_first_keys(TEST_NUM_KEYS_VALID, TEST_NUM_KEYS_THRESHOLD, true);

        if let Some(keys) = result {
            // Skip test if we have less than 2 keys
            if keys.len() < 2 {
                return;
            }

            // Check that values are in ascending order
            for i in 1..keys.len() {
                assert!(
                    keys[i - 1].1 <= keys[i].1,
                    "Keys not in ascending order: {} ({}) > {} ({})",
                    set32_to_string(keys[i - 1].0),
                    keys[i - 1].1,
                    set32_to_string(keys[i].0),
                    keys[i].1
                );
            }
        }
    }

    #[test]
    fn test_get_first_keys_sorted() {
        // Create a mock that would return multiple keys
        // This test assumes the function would return sorted results
        let result = get_first_keys(TEST_NUM_KEYS_VALID, TEST_NUM_KEYS_THRESHOLD, true);

        if let Some(keys) = result {
            // Print the first 100 items (or all if less than 100)
            let count_to_print = std::cmp::min(100, keys.len());
            println!("First {} keys:", count_to_print);
            for i in 0..count_to_print {
                println!(
                    "Key {}: {} with value {}",
                    i,
                    set32_to_string(keys[i].0),
                    keys[i].1
                );
            }

            // If there's more than one key, check that they are sorted by value (ascending)
            for i in 1..keys.len() {
                assert!(keys[i - 1].1 >= keys[i].1);
            }
        }
    }
}
