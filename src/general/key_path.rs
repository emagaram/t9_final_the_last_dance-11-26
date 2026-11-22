use bincode::{Decode, Encode};
use itertools::Itertools;

use crate::{
    general::set32::Set32,
    globals::{LETTER_MAP_ALPHABET, NUM_LETTERS},
    util::{compare_letters, num_arr_to_num, num_to_num_arr_target_digits, path_to_set32},
};

#[derive(Decode, Encode, Clone, PartialEq, Eq, Debug)]
pub struct KeyPath {
    pub value: u128,
    pub keys: Vec<Set32>,
    pub key_set: Set32,
}

impl Default for KeyPath {
    fn default() -> Self {
        Self {
            value: num_arr_to_num(&LETTER_MAP_ALPHABET.map(|v| v as u128), NUM_LETTERS as u128),
            keys: vec![],
            key_set: Set32::EMPTY,
        }
    }
}

impl KeyPath {
    pub fn from_set_path(path: Vec<Set32>) -> Self {
        Self {
            value: Self::keys_to_u128(&path),
            key_set: path_to_set32(&path),
            keys: path,
        }
    }

    pub fn letter_map_to_keys(letter_map: &[u128; NUM_LETTERS]) -> Vec<Set32> {
        let mut keys_map: [Set32; NUM_LETTERS] = [Set32::EMPTY; NUM_LETTERS];
        for (i, &value) in letter_map.iter().enumerate() {
            let letter = (NUM_LETTERS - 1) - i;
            // println!(
            //     "Mapping: {} to {}",
            //     value,
            //     set32_to_string(keys_map[value as usize].add(letter as u32))
            // );
            keys_map[value as usize] = keys_map[value as usize].add(letter as u32);
        }
        keys_map
            .into_iter()
            .filter(|&set| set.count_ones() > 1)
            .sorted_by(|a, b| compare_letters(*b, *a))
            .collect()
    }
    pub fn keys_to_letter_map(keys: &Vec<Set32>) -> [u128; NUM_LETTERS] {
        // TODO create a default map to avoid mapping every time
        let mut map = LETTER_MAP_ALPHABET.map(|v| v as u128);
        for key in keys {
            let ones = key.ones_indices();
            for i in ones.iter().rev() {
                let map_idx = (NUM_LETTERS - 1) - *i as usize;
                let map_value = (NUM_LETTERS - 1) - ones[ones.len() - 1] as usize;
                // println!("Mapping: {} to {}", map_idx, map_value);
                map[map_idx] = map_value as u128;
            }
        }
        map
    }
    pub fn keys_to_u128(keys: &Vec<Set32>) -> u128 {
        let map = Self::keys_to_letter_map(keys);
        // println!("keys to map: {:?}", map);
        num_arr_to_num(&map, NUM_LETTERS as u128)
    }
    pub fn u128_to_keys(value: u128) -> Vec<Set32> {
        let arr = num_to_num_arr_target_digits(value, NUM_LETTERS as u128, NUM_LETTERS);

        let mut keys = [Set32::EMPTY; NUM_LETTERS];
        arr.into_iter()
            .enumerate()
            .for_each(|(letter, letter_key)| {
                let letter = (NUM_LETTERS - 1) - letter;
                keys[letter_key as usize] = keys[letter_key as usize].add(letter as u32);
            });
        keys.into_iter()
            .filter(|set| set.count_ones() > 1)
            .collect()
    }
    pub fn u128_to_keys_sorted(value: u128) -> Vec<Set32> {
        let arr = num_to_num_arr_target_digits(value, NUM_LETTERS as u128, NUM_LETTERS);

        let mut keys = [Set32::EMPTY; NUM_LETTERS];
        arr.into_iter()
            .enumerate()
            .for_each(|(letter, letter_key)| {
                let letter = (NUM_LETTERS - 1) - letter;
                keys[letter_key as usize] = keys[letter_key as usize].add(letter as u32);
            });
        keys.into_iter()
            .filter(|set| set.count_ones() > 1)
            .sorted_by(|a, b| compare_letters(*b, *a))
            .collect()
    }
    pub fn get_u128(keys: &[Option<usize>; NUM_LETTERS]) -> u128 {
        let mut value = 0;
        for (i, key) in keys.iter().enumerate() {
            let key = match key {
                Some(k) => *k,
                None => i,
            };
            value = value * (NUM_LETTERS as u128) + (key as u128);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::{create_set32s_vec, general::key_path::KeyPath};

    #[test]
    fn test_keypath_default() {
        let default = KeyPath::default();
        assert_eq!(default.keys, vec![]);
        assert_eq!(default.key_set, crate::general::set32::Set32::EMPTY);
        // The value should be the default mapping of the alphabet
        let expected_value = crate::util::num_arr_to_num(
            &crate::globals::LETTER_MAP_ALPHABET.map(|v| v as u128),
            crate::globals::NUM_LETTERS as u128,
        );
        assert_eq!(default.value, expected_value);
    }

    #[test]
    fn test_from_set_path_and_equality() {
        let keys = create_set32s_vec!("abc,def,qtz");
        let key_path = KeyPath::from_set_path(keys.clone());
        assert_eq!(key_path.keys, keys);
        assert_eq!(key_path, KeyPath::from_set_path(keys));
    }

    #[test]
    fn test_keys_to_u128_and_back() {
        let keys = create_set32s_vec!("abc,def,qtz");
        let value = KeyPath::keys_to_u128(&keys);
        let keys_back = KeyPath::u128_to_keys(value);
        assert_eq!(keys, keys_back);
    }

    #[test]
    fn test_get_u128_with_some_and_none() {
        let mut arr = [None; crate::globals::NUM_LETTERS];
        for i in 0..crate::globals::NUM_LETTERS {
            arr[i] = Some(i);
        }
        let value = KeyPath::get_u128(&arr);
        let expected = KeyPath::default().value;
        assert_eq!(value, expected);

        // Mix Some and None
        let mut arr2 = [None; crate::globals::NUM_LETTERS];
        arr2[0] = Some(1);
        arr2[1] = Some(0);
        let value2 = KeyPath::get_u128(&arr2);
        assert_ne!(value2, expected);
    }

    #[test]
    fn test_letter_map_to_keys_and_back() {
        let map = crate::globals::LETTER_MAP_ALPHABET.map(|v| v as u128);
        let keys = KeyPath::letter_map_to_keys(&map);
        let map_back = KeyPath::keys_to_letter_map(&keys);
        assert_eq!(map, map_back);
    }

    #[test]
    fn test_speed_clone_vs_create() {
        let arr: Vec<u32> = (1..=100).collect();
        let now = Instant::now();
        let cloned = arr.clone();
        let clone_duration = now.elapsed();

        let now2 = Instant::now();
        let created: Vec<u32> = (1..=100).collect();
        let create_duration = now2.elapsed();

        // Both should be equal
        assert_eq!(cloned, created);

        // Print durations for manual inspection (not for assert)
        println!(
            "Clone duration: {:?}, Create duration: {:?}",
            clone_duration, create_duration
        );
    }
    #[test]
    fn test_keys_to_letter_map() {
        use crate::create_set32s_vec;

        let keys = create_set32s_vec!("abc,def,qtz");
        let map = KeyPath::keys_to_letter_map(&keys);
        assert_eq!(
            map,
            [
                0, 0, 0, 3, 3, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 16, 20, 21, 22,
                23, 24, 16, 26
            ]
        );
        let keys_again = KeyPath::letter_map_to_keys(&map);
        assert_eq!(keys, keys_again);

        let keys_out_of_order = create_set32s_vec!("def,qtz,abc");
        let map_out_of_order = KeyPath::keys_to_letter_map(&keys_out_of_order);
        assert_eq!(map_out_of_order, map);
        let keys_again_out_of_order = KeyPath::letter_map_to_keys(&map_out_of_order);
        assert_eq!(keys_again_out_of_order, keys_again);

        let keys1 = create_set32s_vec!("abcp,def,qtz");
        let keys2 = create_set32s_vec!("def,qtz,abcp");
        let map1 = KeyPath::keys_to_letter_map(&keys1);
        let map2 = KeyPath::keys_to_letter_map(&keys2);
        assert_eq!(map1, map2);
    }

    #[test]
    fn test_keys_from_u128() {
        let keys = create_set32s_vec!("abc,def,qtz");
        // println!("{}", set32s_to_string(&keys));
        let value = KeyPath::keys_to_u128(&keys);
        let keys_again = KeyPath::u128_to_keys(value);
        assert_eq!(keys, keys_again);
    }
}
