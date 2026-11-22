use std::{fs::File, io::BufReader};

use gxhash::{HashMap, HashMapExt};

use crate::{general::set32::Set32, globals::ASCII_CODE_TO_INDEX};

pub type FreqList = Vec<(Vec<u8>, f32)>;
pub type FreqList64 = Vec<(Vec<u8>, f64)>;
pub type FreqListsFlattened = Vec<(u8, Vec<u8>)>;
pub type EnhancedFreqListsFlattened = Vec<(u8, Vec<u8>)>;

pub fn create_freq_list32(dict_path: &str) -> FreqList {
    let file = File::open(dict_path).expect("file not found");
    let reader = BufReader::new(file);
    let word_frequencies: HashMap<String, f32> =
        serde_json::from_reader(reader).expect("read json properly");
    let mut freq_list: FreqList = word_frequencies
        .into_iter() // Use `.into_iter()` to consume the hashmap and avoid cloning the keys
        .map(|(key, value)| {
            (
                key.into_bytes()
                    .iter()
                    .map(|b| ASCII_CODE_TO_INDEX[b])
                    .collect(),
                value,
            )
        }) // Convert each key into a Vec<u8>
        .collect();
    freq_list.sort_by(|a, b| {
        b.1.partial_cmp(&a.1) // First, try to compare the values
            .unwrap_or(std::cmp::Ordering::Equal) // In case of NaNs or partial comparison failure
            .then_with(|| a.0.cmp(&b.0)) // If values are equal, sort by keys in ascending order
    });
    freq_list
}

pub fn create_freq_list(dict_path: &str) -> FreqList {
    create_freq_list64(dict_path)
        .iter()
        .map(|(k, v)| (k.clone(), *v as f32))
        .collect()
}

pub fn create_freq_list64(dict_path: &str) -> FreqList64 {
    let file = File::open(dict_path).expect("file not found");
    let reader = BufReader::new(file);
    let word_frequencies: HashMap<String, f64> =
        serde_json::from_reader(reader).expect("read json properly");
    let mut freq_list: FreqList64 = word_frequencies
        .into_iter() // Use `.into_iter()` to consume the hashmap and avoid cloning the keys
        .map(|(key, value)| {
            (
                key.into_bytes()
                    .iter()
                    .map(|b| ASCII_CODE_TO_INDEX[b])
                    .collect(),
                value,
            )
        }) // Convert each key into a Vec<u8>
        .collect();
    freq_list.sort_by(|a, b| {
        b.1.partial_cmp(&a.1) // First, try to compare the values
            .unwrap_or(std::cmp::Ordering::Equal) // In case of NaNs or partial comparison failure
            .then_with(|| a.0.cmp(&b.0)) // If values are equal, sort by keys in ascending order
    });
    freq_list
}

pub fn filter_enhanced_freq_list(
    freq_list: &EnhancedFreqListsFlattened,
    letters: Set32,
) -> EnhancedFreqListsFlattened {
    let mut result: EnhancedFreqListsFlattened = Vec::new();
    for (len, words) in freq_list {
        let mut filtered_words: Vec<u8> = Vec::new();
        let word_size = *len;
        let letters_size = 4;
        let occurence_size = 1;
        let score_size = 4;
        let inc = word_size + letters_size + occurence_size + score_size;
        let letters_start = word_size;
        for chunk in words.chunks(inc.into()) {
            let letters_bytes = &chunk[letters_start as usize..(letters_start as usize) + 4];
            let word_letters = u32::from_le_bytes([
                letters_bytes[0],
                letters_bytes[1],
                letters_bytes[2],
                letters_bytes[3],
            ]);
            if word_letters & letters.0 == 0 {
                continue;
            }
            filtered_words.extend_from_slice(&chunk);
        }
        result.push((*len, filtered_words));
    }
    result
}

pub fn create_enhanced_freqs_list(freq_list: FreqList) -> EnhancedFreqListsFlattened {
    let mut result: HashMap<u8, Vec<(Vec<u8>, f32)>> = HashMap::new();

    // Group by length
    for (word, score) in freq_list {
        let len = word.len();
        result
            .entry(len as u8)
            .and_modify(|words| words.push((word.clone(), score)))
            .or_insert_with(|| vec![(word, score)]);
    }

    // Convert to sorted vector and flatten each group
    let mut grouped: Vec<(u8, Vec<u8>)> = result
        .into_iter()
        .map(|(len, words)| {
            let mut flattened = Vec::new();
            for (word, score) in words {
                let occurence = 0 as u8;
                let score_bytes = score.to_le_bytes();
                let mut letters = Set32::EMPTY;
                for char in &word {
                    letters = letters.add(*char as u32);
                }
                flattened.extend_from_slice(&word);
                flattened.extend_from_slice(&letters.0.to_le_bytes());
                flattened.push(occurence);
                // flattened.extend_from_slice(&[occurence]);
                flattened.extend_from_slice(&score_bytes);
            }
            (len, flattened)
        })
        .collect();

    grouped.sort_by_key(|(len, _)| *len);
    grouped
}

pub fn group_by_length_flattened(freq_list: FreqList) -> FreqListsFlattened {
    let mut result: HashMap<u8, Vec<(Vec<u8>, f32)>> = HashMap::new();

    // Group by length
    for (word, score) in freq_list {
        let len = word.len();
        result
            .entry(len as u8)
            .and_modify(|words| words.push((word.clone(), score)))
            .or_insert_with(|| vec![(word, score)]);
    }

    // Convert to sorted vector and flatten each group
    let mut grouped: Vec<(u8, Vec<u8>)> = result
        .into_iter()
        .map(|(len, words)| {
            let mut flattened = Vec::new();
            for (word, score) in words {
                // Add the word bytes
                flattened.extend_from_slice(&word);

                // Convert f32 to bytes and add them
                let score_bytes = score.to_le_bytes();
                flattened.extend_from_slice(&score_bytes);
            }
            (len, flattened)
        })
        .collect();

    grouped.sort_by_key(|(len, _)| *len);
    grouped
}

#[cfg(test)]
mod tests {
    use hashbrown::HashMap;

    use crate::general::freq_list::group_by_length_flattened;

    use super::create_freq_list;
    #[test]
    fn freq_list_has_right_num_bytes() {
        let list = create_freq_list("./word_freq_small.json");
        assert_eq!(list[0].0.len(), 3);
        assert_eq!(list[1].0.len(), 4);
        assert_eq!(list[2].0.len(), 10);
    }
    #[test]
    fn freq_list_init_same() {
        let list1 = create_freq_list("./word_freq.json");
        let list2 = create_freq_list("./word_freq.json");
        assert_eq!(list1[1000].0, list2[1000].0);
        assert_eq!(list1[5000].0, list2[5000].0);
        assert_eq!(list1[50000].0, list2[50000].0);
    }
    #[test]
    fn count_num_word_len() {
        let list1 = create_freq_list("./word_freq.json");
        let mut len_track: HashMap<usize, usize> = HashMap::new();
        for word in list1 {
            len_track
                .entry(word.0.len())
                .and_modify(|value| *value += 1)
                .or_insert(1);
        }
        let mut total = 0;
        for key in 1..50 as usize {
            if !len_track.contains_key(&key) {
                continue;
            }
            let np = len_track[&key] * (len_track[&key] - 1) / 2;
            total += np;
            println!("Len: {}, Num {}, num pairs: {}", key, len_track[&key], np);
        }
        println!("Total {}", total);
    }

    #[test]
    fn test_group_by_length_flattened() {
        // Create a simple frequency list for testing
        let freq_list = vec![
            (vec![1, 2, 3], 0.5),            // 3-letter word with score 0.5
            (vec![4, 5, 6, 7], 0.3),         // 4-letter word with score 0.3
            (vec![8, 9, 10], 0.7),           // 3-letter word with score 0.7
            (vec![11, 12, 13, 14, 15], 0.2), // 5-letter word with score 0.2
        ];

        // Call the function
        let result = group_by_length_flattened(freq_list);

        // Verify the result
        assert_eq!(result.len(), 3); // Should have 3 different lengths (3, 4, 5)

        // Check that lengths are sorted
        assert_eq!(result[0].0, 3);
        assert_eq!(result[1].0, 4);
        assert_eq!(result[2].0, 5);

        // Check the first group (length 3)
        let len_3_data = &result[0].1;
        assert!(len_3_data.len() == 14); // 2 words * (3 bytes + 4 bytes for f32)

        // Check the second group (length 4)
        let len_4_data = &result[1].1;
        assert!(len_4_data.len() == 8); // 1 word * (4 bytes + 4 bytes for f32)

        // Check the third group (length 5)
        let len_5_data = &result[2].1;
        assert!(len_5_data.len() == 9); // 1 word * (5 bytes + 4 bytes for f32)
    }
}
