use std::{cmp::Ordering, fs::File};

use crate::{
    general::set32::Set32,
    globals::{INDEX_TO_CHAR, NUM_LETTERS, SET32_SIZE},
};

pub const ALPHABET_SET32: Set32 = Set32((1 << NUM_LETTERS) - 1);

#[macro_export]
macro_rules! create_btreeset {
    ($($item:expr_2021),* $(,)?) => {
        {
            let mut set = BTreeSet::new();
            $(
                set.insert($item);
            )*
            set
        }
    };
}

#[macro_export]
macro_rules! create_word {
    ($word:expr_2021) => {{
        use crate::globals::CHAR_TO_INDEX;
        let mut result: Vec<u8> = Vec::new();
        for ch in $word.chars() {
            result.push(*CHAR_TO_INDEX.get(&ch).unwrap_or(&0));
        }
        result
    }};
}

#[macro_export]
macro_rules! create_set32 {
    ($($ch:expr_2021),*) => {{
            use crate::globals::CHAR_TO_INDEX;
            let mut res = Set32::EMPTY;
            for ch in [$($ch),*] {
                res = res.add(CHAR_TO_INDEX[&ch] as u32);
            }
            res
    }};
}

#[macro_export]
macro_rules! create_set32_str {
    ($chars:expr_2021) => {{
        use crate::general::set32::Set32;
        use crate::globals::CHAR_TO_INDEX;
        let mut res = Set32::EMPTY;
        for ch in $chars.chars() {
            res = res.add(CHAR_TO_INDEX[&ch] as u32);
        }
        res
    }};
}
#[macro_export]
macro_rules! create_set32s_vec {
    ($chars:expr_2021) => {{
        use crate::general::set32::Set32;
        use crate::globals::CHAR_TO_INDEX;
        let sets = $chars
            .split(',')
            .map(|ch| {
                let mut set = Set32::EMPTY;
                for c in ch.trim().chars() {
                    set = set.add(CHAR_TO_INDEX[&c] as u32);
                }
                set
            })
            .collect::<Vec<Set32>>();
        sets
    }};
}

pub fn get_num_read_workers(cpu_count: usize, write_read_ratio: usize) -> usize {
    let mut num_cpus = cpu_count as f64;
    num_cpus -= 2.0; // Reserve 2 for tracing
    num_cpus -= 1.0; // Write thread
    num_cpus *= write_read_ratio as f64 / (write_read_ratio + 1) as f64;
    if num_cpus < 1.0 {
        return 1;
    }
    num_cpus as usize
}

pub fn get_num_calc_workers(cpu_count: usize, write_read_ratio: usize) -> usize {
    let mut num_cpus = cpu_count as f64;
    num_cpus -= 2.0; // Reserve 2 for tracing
    num_cpus -= 1.0; // Write thread
    num_cpus *= 1.0 / (write_read_ratio + 1) as f64;
    if num_cpus < 1.0 {
        return 1;
    }
    num_cpus as usize
}

pub fn path_to_set32(path: &[Set32]) -> Set32 {
    let mut result = Set32::EMPTY;
    for key in path {
        result = result.union(*key);
    }
    result
}

// pub(crate) use create_set32;
pub fn bits_to_num_bytes(num_bits: usize) -> usize {
    (num_bits + 7) / 8
}

pub fn bits_to_byte_index(num_bits: usize) -> usize {
    num_bits / 8
}

pub fn u8slice_to_set(slice: &[u8]) -> Set32 {
    Set32(u32::from_ne_bytes(slice.try_into().unwrap()))
}

pub fn set_to_u8slice(set: &Set32) -> [u8; 4] {
    set.value().to_ne_bytes()
}

pub fn set32_to_string(s: Set32) -> String {
    let mut res: String = String::new();
    for index in s.ones_indices() {
        res.push_str(INDEX_TO_CHAR[&index.try_into().unwrap()].as_str());
    }
    res.chars().rev().collect()
}

pub fn set32s_to_string(sets: &[Set32]) -> String {
    let mut res: String = String::new();
    for s in sets {
        for index in s.ones_indices().iter().rev() {
            res.push_str(INDEX_TO_CHAR[&(*index as u8)].as_str());
        }
        res.push(' ');
    }
    res.pop();
    res
}

pub fn u8s_to_sets(u8s: &Vec<u8>) -> Vec<Set32> {
    let mut ptr = 0;
    let mut res: Vec<Set32> = Vec::new();
    while ptr < u8s.len() {
        let slice = &u8s[ptr..ptr + 4];
        let bytes: [u8; SET32_SIZE] = slice.try_into().expect("Slice with incorrect size");
        res.push(Set32(u32::from_ne_bytes(bytes)));
        ptr += SET32_SIZE;
    }
    res
}

pub fn compare_letters(probe: Set32, other: Set32) -> Ordering {
    let ones_probe = probe.count_ones();
    let ones_target = other.count_ones();
    if ones_probe > ones_target {
        return Ordering::Greater;
    } else if ones_probe < ones_target {
        return Ordering::Less;
    } else if probe > other {
        return Ordering::Greater;
    } else if probe < other {
        return Ordering::Less;
    }
    return Ordering::Equal;
}

pub fn compare_paths(probe: &[Set32], other: &[Set32]) -> Ordering {
    let mut probe_iter = probe.iter();
    let mut other_iter = other.iter();

    loop {
        match (probe_iter.next(), other_iter.next()) {
            (Some(probe), Some(other)) => {
                let cmp = compare_letters(*probe, *other);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

pub fn compare_letters_by_value(probe: Set32, other: Set32) -> Ordering {
    if probe == other {
        Ordering::Equal
    } else if probe > other {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

pub fn path_to_string(path: &[Set32]) -> String {
    let mut res = String::new();
    for s in path {
        // each s is a singleton set; convert to its char
        res.push_str(&set32_to_string(*s));
    }
    res
}

pub fn num_to_num_arr_target_digits<T>(mut num: T, base: T, target_digits: usize) -> Vec<T>
where
    T: Copy + Default + PartialEq + std::ops::Div<Output = T> + std::ops::Rem<Output = T>,
{
    if num == T::default() {
        return vec![T::default()];
    }

    let mut digits = Vec::new();
    while num != T::default() {
        digits.push(num % base);
        num = num / base;
    }
    while digits.len() < target_digits {
        digits.push(T::default());
    }

    // Reverse since we built it backwards (least significant digit first)
    digits.reverse();
    digits
}

pub fn num_arr_to_num<T>(layout: &[T], base: T) -> T
where
    T: Copy + Default + std::ops::Mul<Output = T> + std::ops::Add<Output = T>,
{
    layout.iter().fold(T::default(), |acc, x| acc * base + *x)
}

/// Converts a layout (iterator of numbers) to a single number using the given base.
/// Works with any number type that implements Copy + Into<u128>.
pub fn num_to_num_arr<T>(mut num: T, base: T) -> Vec<T>
where
    T: Copy + Default + PartialEq + std::ops::Div<Output = T> + std::ops::Rem<Output = T>,
{
    if num == T::default() {
        return vec![T::default()];
    }

    let mut digits = Vec::new();
    while num != T::default() {
        digits.push(num % base);
        num = num / base;
    }

    // Reverse since we built it backwards (least significant digit first)
    digits.reverse();
    digits
}

pub fn get_set_file_deep(path: &str, options: &std::fs::OpenOptions) -> Result<File, ()> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        if std::fs::create_dir_all(parent).is_err() {
            eprintln!("Failed to create directory: {}", parent.display());
            return Err(());
        }
    }
    match options.open(&path) {
        Ok(file) => Ok(file),
        Err(_) => Err(()),
    }
}

#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use core::num;

    use super::compare_letters;
    use crate::create_set32_str;
    use crate::general::set32::Set32;
    use crate::globals::CHAR_TO_INDEX;
    use crate::util::{bits_to_byte_index, bits_to_num_bytes, u8s_to_sets};

    #[test]
    fn test_get_cpus() {
        let write_read_ratio = 3;
        let cpu_count = 32;
        let num_read_workers = super::get_num_read_workers(cpu_count, write_read_ratio);
        let num_write_workers = super::get_num_calc_workers(cpu_count, write_read_ratio);
        println!(
            "CPU count: {}, Read workers: {}, Write workers: {}",
            cpu_count, num_read_workers, num_write_workers
        );
        assert!(num_read_workers > 0);
        assert!(num_write_workers > 0);
    }

    #[test]
    fn test_num_arr_to_num() {
        let layout = vec![1, 2, 3, 4, 5];
        let base = 10;
        let result = super::num_arr_to_num(&layout, base);
        assert_eq!(result, 12345);
        let layout = vec![0, 0, 3, 0, 5];
        let result = super::num_arr_to_num(&layout, base);
        assert_eq!(result, 305);
    }

    #[test]
    fn test_num_to_num_arr() {
        let num = 12345;
        let base = 10;
        let result = super::num_to_num_arr(num, base);
        assert_eq!(result, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn util_bits_to_bytes() {
        assert_eq!(bits_to_num_bytes(16), 2);
        assert_eq!(bits_to_num_bytes(15), 2);
        assert_eq!(bits_to_num_bytes(8), 1);
        assert_eq!(bits_to_num_bytes(7), 1);
    }

    #[test]
    fn util_bits_to_byte_index() {
        assert_eq!(bits_to_byte_index(16), 2);
        assert_eq!(bits_to_byte_index(15), 1);
        assert_eq!(bits_to_byte_index(8), 1);
        assert_eq!(bits_to_byte_index(7), 0);
    }
    #[test]
    fn util_u8s_to_sets_has_4_items_when_u8s_has_16() {
        let u8s = &vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let res = u8s_to_sets(&u8s);
        assert_eq!(res.len(), 4);
    }
    #[test]
    fn util_u8s_to_sets_has_right_values_when_not_empty() {
        let u8s = &vec![1, 0, 0, 0, 2, 0, 0, 0];
        let res = u8s_to_sets(&u8s);
        assert_eq!(res[0].value(), 1);
        assert_eq!(res[1].value(), 2);
    }
}
