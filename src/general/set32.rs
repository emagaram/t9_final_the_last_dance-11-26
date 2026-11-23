use std::iter;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::util::set32_to_string;

#[derive(
    PartialEq, Eq, PartialOrd, Debug, Clone, Copy, Hash, Ord, Serialize, Deserialize, Decode, Encode,
)]
pub struct Set32(pub u32);

impl Set32 {
    pub const EMPTY: Set32 = Set32(0);
    pub const MAX_SIZE: u32 = 32;
    pub const MAX_ITEM_VALUE: u32 = 31;
    pub const MIN_ITEM_VALUE: u32 = 0;
    pub fn value(&self) -> u32 {
        self.0
    }
    pub fn ones_indices(&self) -> Vec<u32> {
        self.into_iter().collect()
    }
    pub fn fill_from_right(count: u32) -> Set32 {
        debug_assert!(count <= Self::MAX_SIZE);
        let bits = match count {
            32 => !0,
            count => (1 << count) - 1,
        };
        Set32(bits)
    }
    pub fn fill_from_left(count: u32) -> Set32 {
        debug_assert!(count <= Self::MAX_SIZE);
        let bits = match count {
            0 => 0,
            32 => !0,
            count => ((1 << count) - 1) << Self::MAX_SIZE - count,
        };
        Set32(bits)
    }
    pub fn add(&self, bit: u32) -> Set32 {
        debug_assert!(bit <= Self::MAX_ITEM_VALUE);
        Set32(self.0 | 1 << bit)
    }

    pub fn singleton(bit: u32) -> Set32 {
        debug_assert!(bit <= Self::MAX_ITEM_VALUE);
        Set32(1 << bit)
    }

    pub fn contains(&self, bit: u32) -> bool {
        debug_assert!(bit <= Self::MAX_ITEM_VALUE);
        self.0 & (1 << bit) != 0
    }

    pub fn count_ones(&self) -> u32 {
        self.0.count_ones()
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn difference(&self, other: Set32) -> Set32 {
        Set32(self.0 & !other.0)
    }

    pub fn remove(&self, bit: u32) -> Set32 {
        debug_assert!(bit <= Self::MAX_ITEM_VALUE);
        Set32(self.0 & !(1 << bit))
    }

    pub fn union(&self, other: Set32) -> Set32 {
        Set32(self.0 | other.0)
    }

    pub fn intersect(&self, other: Set32) -> Set32 {
        Set32(self.0 & other.0)
    }

    pub fn max_item(&self) -> Option<u32> {
        match self.0.leading_zeros() {
            u32::BITS => None,
            n => Some(u32::BITS - n - 1),
        }
    }

    pub fn min_item(&self) -> Option<u32> {
        match self.0.trailing_zeros() {
            32 => None,
            n => Some(n),
        }
    }
    pub fn max_zero(&self, alphabet_set: Set32) -> Option<u32> {
        match (self.0 | !alphabet_set.0).leading_ones() {
            u32::BITS => None,
            n => Some(u32::BITS - n - 1),
        }
    }

    pub fn min_zero(&self) -> Option<u32> {
        match self.0.trailing_ones() {
            32 => None,
            n => Some(n),
        }
    }

    // https://www.geeksforgeeks.org/next-higher-number-with-same-number-of-set-bits
    fn same_ones_count(count: u32) -> impl Iterator<Item = i64> {
        debug_assert!(count >= 1 && count <= 32);
        let mut n: i64 = (1 << count) - 1;
        let max_bits = u32::BITS;
        let expected_max = ((1 << count) - 1) << (max_bits - count);
        let next = move || {
            let result = n;
            let right_one = n & (-n);
            let next_higher_one_bit = n + right_one;
            let right_ones_pattern = n ^ next_higher_one_bit;
            let right_ones_pattern = right_ones_pattern / right_one;
            let right_ones_pattern = right_ones_pattern >> 2;
            n = next_higher_one_bit | right_ones_pattern;
            match result <= expected_max {
                true => Some(result),
                false => None,
            }
        };
        let iterator = iter::from_fn(next);
        iterator
    }
    /// Converts the set into a `Vec<u32>` by consuming it.
    /// This method takes ownership of the set, iterates over all its elements,
    /// and collects them into a new `Vec<u32>` in iterator order.
    pub fn subsets_of_size(&self, size: u32) -> impl Iterator<Item = Set32> + use<> {
        debug_assert!(size <= Self::MAX_SIZE, "subset size is too big");
        debug_assert!(size > 0, "the size of subset must be bigger than 0");

        let items = self.into_iter().collect::<Vec<u32>>();
        let items_count = items.len();
        let max_exclusive = 1 << items_count;
        debug_assert!(items_count >= size as usize);
        Set32::same_ones_count(size)
            .take_while(move |i| *i < max_exclusive)
            .map(move |i| {
                Set32(i as u32).into_iter().fold(Set32::EMPTY, |total, i| {
                    let aa = items[i as usize];
                    total.add(aa)
                })
            })
    }

    pub fn zero_it() -> impl Iterator<Item = Set32> + use<> {
        iter::once(Set32::EMPTY)
    }

    pub fn to_string(&self) -> String {
        format!("{}", set32_to_string(*self))
    }
}

impl Iterator for Set32 {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        match self.0 {
            0 => None,
            _ => {
                let trailing_zeros = self.0.trailing_zeros();
                self.0 = self.0 ^ (1 << (trailing_zeros));
                Some(trailing_zeros)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_set32_str, globals::CHAR_TO_INDEX, util::ALPHABET_SET32};
    use std::time::Instant;

    #[test]
    fn test_max_zero() {
        let set32 = create_set32_str!("abcd");
        assert_eq!(
            set32.max_zero(ALPHABET_SET32),
            Some(*CHAR_TO_INDEX.get(&'e').unwrap() as u32)
        );
        let set32 = create_set32_str!("");
        assert_eq!(
            set32.max_zero(ALPHABET_SET32),
            Some(*CHAR_TO_INDEX.get(&'a').unwrap() as u32)
        );
    }

    #[test]
    fn test_min_zero() {
        let set32 = create_set32_str!("abcd");
        assert_eq!(
            set32.min_zero(),
            Some(*CHAR_TO_INDEX.get(&'\'').unwrap() as u32)
        );
        let set32 = create_set32_str!("abcd'");
        assert_eq!(
            set32.min_zero(),
            Some(*CHAR_TO_INDEX.get(&'z').unwrap() as u32)
        );
    }

    #[test]
    fn test_iter() {
        let set32 = ALPHABET_SET32;
        let mut res = Set32::EMPTY;
        for letter in set32.into_iter() {
            res = res.union(Set32::singleton(letter));
        }
        assert_eq!(res, ALPHABET_SET32);
    }

    fn string_to_bits(s: &str) -> Set32 {
        let s = s.trim();
        if s == "" {
            Set32::EMPTY
        } else {
            s.split(",")
                .map(|i| {
                    i.trim()
                        .parse::<u32>()
                        .expect("could not convert the string to a u32")
                })
                .collect::<Vec<u32>>()
                .into_iter()
                .fold(Set32::EMPTY, |total, i| total.add(i))
        }
    }

    #[test]
    fn into_iterator() {
        assert_eq!(Set32::EMPTY.into_iter().count(), 0);
        assert_eq!(Set32::EMPTY.add(1).add(2).into_iter().count(), 2);
        assert_eq!(
            Set32::EMPTY
                .add(5)
                .add(3)
                .into_iter()
                .fold(0, |total, i| total + i),
            8,
        );
    }
    #[test]
    fn set32_fill_from_left_when_zero() {
        assert_eq!(Set32::fill_from_left(0), Set32::EMPTY);
    }
    #[test]
    fn set32_fill_from_left_when_max() {
        assert_eq!(
            Set32::fill_from_left(32).ones_indices(),
            (0..32).collect::<Vec<u32>>()
        );
    }
    #[test]
    fn set32_fill_from_left_when_not_zero() {
        assert_eq!(
            Set32::fill_from_left(30).ones_indices(),
            (2..32).collect::<Vec<u32>>()
        );
        assert_eq!(Set32::fill_from_left(1).ones_indices(), vec![31]);
    }
    #[test]
    fn fill_when_zero() {
        assert_eq!(Set32::fill_from_right(0), Set32::EMPTY);
    }

    #[test]
    fn fill_when_max() {
        assert_eq!(
            Set32::fill_from_right(32).ones_indices(),
            (0..32).collect::<Vec<u32>>()
        );
    }

    #[test]
    fn fill_when_not_zero() {
        assert_eq!(Set32::fill_from_right(1).ones_indices(), vec![0]);
        assert_eq!(Set32::fill_from_right(2).ones_indices(), vec![0, 1]);
    }

    #[test]
    fn add() {
        let zero = Set32::EMPTY;
        assert_eq!(zero.ones_indices(), Vec::<u32>::new());
        assert_eq!(zero.add(3).add(8).ones_indices(), vec![3, 8]);
        assert_eq!(zero.add(31).ones_indices(), vec![31]);
    }

    // #[test]
    // #[should_panic]
    // fn add_panic_if_invalid_index() {
    //     Set32::EMPTY.add(32);
    // }

    #[test]
    fn difference() {
        let data = [
            ("", "", ""),
            ("1", "1", ""),
            ("1,2,3", "2,3", "1"),
            ("5,6,7", "1", "5,6,7"),
            ("23,1,8,3,4", "8,1", "3,4,23"),
        ];
        fn test(start: &str, except: &str, expected: &str) -> () {
            let start = string_to_bits(start);
            let other = string_to_bits(except);
            let actual = start.difference(other);
            let expected = string_to_bits(expected);
            assert_eq!(actual, expected);
        }
        data.into_iter().for_each(|(a, b, c)| test(a, b, c));
    }

    #[test]
    fn remove_test() {
        fn test(bits: &str, item: u32, expected: &str) {
            assert_eq!(string_to_bits(bits).remove(item), string_to_bits(expected));
        }
        test("", 0, "");
        test("", 1, "");
        test("", 31, "");
        test("0", 0, "");
        test("0", 1, "0");
        test("0,1,2", 1, "0,2");
        test("0,1,2,3,4,5", 5, "0,1,2,3,4");
        test("0,1,2,3,4,5,31", 31, "0,1,2,3,4,5");
        test("0,1,2,3,4,5,31", 0, "1,2,3,4,5,31");
    }

    #[test]
    fn to_string_test() {
        // Letters in alphabetical order (bit 26 = a ... bit 1 = z, bit 0 = ')
        let data = [
            ("", ""),
            ("1", "z"),
            ("1,2,3", "xyz"),
            ("5,4,3", "vwx"),
            ("15,4,0", "lw'"),
            ("26,25,24", "abc"),
        ];
        fn test(start: &str, expected: &str) -> () {
            let start = string_to_bits(start);
            let actual = start.to_string();
            assert_eq!(actual, expected);
        }
        data.into_iter()
            .for_each(|(start, expected)| test(start, expected));
    }

    #[test]
    fn contains() {
        assert_eq!(Set32::EMPTY.contains(3), false);
        assert_eq!(Set32::EMPTY.add(1).contains(2), false);
        assert_eq!(Set32::EMPTY.add(1).add(2).contains(1), true);
    }

    #[test]
    fn count_test() {
        assert_eq!(Set32::EMPTY.count_ones(), 0);
        assert_eq!(Set32::EMPTY.add(1).count_ones(), 1);
        assert_eq!(Set32::EMPTY.add(1).add(2).count_ones(), 2);
        assert_eq!(Set32::fill_from_right(32).count_ones(), 32);
    }

    #[test]
    fn singleton_test() {
        assert_eq!(Set32::singleton(0).ones_indices(), vec![0]);
        assert_eq!(Set32::singleton(31).ones_indices(), vec![31]);
        assert_eq!(Set32::singleton(5).ones_indices(), vec![5]);
    }

    #[test]
    fn union() {
        let data = [
            ("", "", ""),
            ("1", "1", "1"),
            ("1,2,3", "2,3", "1,2,3"),
            ("5,6,7", "1,2,3", "1,2,3,5,6,7"),
            ("2", "1", "1,2"),
            ("", "5", "5"),
        ];
        fn test(start: &str, except: &str, expected: &str) -> () {
            let start = string_to_bits(start);
            let other = string_to_bits(except);
            let actual = start.union(other);
            let expected = string_to_bits(expected);
            assert_eq!(actual, expected);
        }
        data.into_iter().for_each(|(a, b, c)| test(a, b, c));
    }

    #[test]
    fn intersect() {
        let data = [
            ("", "", ""),
            ("1", "1", "1"),
            ("1,2,3", "2,3", "2,3"),
            ("5,6,7", "1,2,3", ""),
            ("1,2,3,4,5", "1,2,3,4,5", "1,2,3,4,5"),
            ("0,31", "5,31", "31"),
            ("2", "1,2", "2"),
            ("", "5", ""),
        ];
        fn test(start: &str, except: &str, expected: &str) -> () {
            let start = string_to_bits(start);
            let other = string_to_bits(except);
            let actual = start.intersect(other);
            let expected = string_to_bits(expected);
            assert_eq!(actual, expected);
        }
        data.into_iter().for_each(|(a, b, c)| test(a, b, c));
    }

    #[test]
    fn max_item() {
        assert_eq!(Set32::EMPTY.max_item(), None);
        assert_eq!(Set32::EMPTY.add(0).max_item(), Some(0));
        assert_eq!(Set32::EMPTY.add(0).add(1).max_item(), Some(1));
        assert_eq!(Set32::EMPTY.add(5).max_item(), Some(5));
        assert_eq!(Set32::EMPTY.add(5).add(17).add(3).max_item(), Some(17));
        assert_eq!(Set32::EMPTY.add(31).max_item(), Some(31));
        assert_eq!(Set32::EMPTY.add(31).add(5).max_item(), Some(31));
        assert_eq!(Set32::fill_from_right(32).max_item(), Some(31));
    }

    #[test]
    fn min_item() {
        assert_eq!(Set32::EMPTY.min_item(), None);
        assert_eq!(Set32::EMPTY.add(0).min_item(), Some(0));
        assert_eq!(Set32::EMPTY.add(0).add(1).min_item(), Some(0));
        assert_eq!(Set32::EMPTY.add(5).min_item(), Some(5));
        assert_eq!(Set32::EMPTY.add(5).add(17).add(3).min_item(), Some(3));
        assert_eq!(Set32::EMPTY.add(31).min_item(), Some(31));
    }

    #[test]
    fn same_ones_count_ends_at_max() {
        for expected_ones in [1, 5, 9, 12, 32] {
            let max_bits = 32;
            let expected_max = ((1 << expected_ones) - 1) << (max_bits - expected_ones);
            let actual_max = Set32::same_ones_count(expected_ones).last().unwrap_or(-1);
            assert_eq!(actual_max, expected_max);
        }
    }

    #[test]
    fn with_ones_count_test() {
        for expected_ones in [1, 5, 9, 12, 23, 31, 32] {
            let all_correct_ones = Set32::same_ones_count(expected_ones).take(1000).all(|n| {
                let actual_ones = Set32(n as u32).into_iter().count();
                actual_ones == (expected_ones as usize)
            });
            assert!(all_correct_ones)
        }
    }

    fn choose_count(n: u32, k: u32) -> u128 {
        let n = n as u128;
        let k = k as u128;
        fn factorial(n: u128) -> u128 {
            (1..=n).product()
        }
        factorial(n) / factorial(n - k) / factorial(k)
    }

    #[test]
    fn subsets_of_size() {
        fn test(items: &str) {
            let bits = string_to_bits(items);
            let ones_count = bits.into_iter().count();

            // subsets have correct number of items (no duplicates)
            (1..ones_count).for_each(|subset_size| {
                let actual_size = bits.subsets_of_size(subset_size as u32).count();
                let expected_count = choose_count(ones_count as u32, subset_size as u32);
                assert_eq!(actual_size, expected_count as usize);
            });

            // subsets items are unique
            (1..ones_count).for_each(|subset_size| {
                let set = bits
                    .subsets_of_size(subset_size as u32)
                    .collect::<std::collections::HashSet<Set32>>();
                let expected_count = choose_count(ones_count as u32, subset_size as u32);
                assert_eq!(set.len(), expected_count as usize);
            });

            // subsets items are all in the source bits
            (1..ones_count).for_each(|subset_size| {
                let all_valid_items = bits.subsets_of_size(subset_size as u32).all(move |subset| {
                    let m = subset.difference(bits) == Set32::EMPTY;
                    m
                });
                assert!(all_valid_items)
            });
        }
        let data = [
            "0,1,5,7",
            "2,4,10,30",
            "1,2,3,4,5,6,7,30",
            "1,2,3,4,5,6,7,8,9,10,12,13,14",
            "6,1,8,7,2,9",
            "1",
            "1,2",
        ];
        data.iter().for_each(|s| {
            test(&s);
        })
    }

    #[test]
    fn subsets_print_out() {
        Set32::fill_from_right(6)
            .remove(3)
            .subsets_of_size(3)
            .for_each(|i| println!("{:?}", i.to_string()));
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_ones_indices_vs_subtracting -- --nocapture
    #[test]
    fn test_ones_indices_vs_subtracting() {
        // Create a complex set with various bits set
        let complex_set = Set32::fill_from_right(20)
            .difference(Set32::fill_from_right(5))
            .add(25)
            .add(30);

        // Test ones_indices method
        let start = Instant::now();
        for _ in 0..100_000 {
            let indices = complex_set.ones_indices();
            assert!(indices.len() > 0);
        }
        let ones_indices_duration = start.elapsed();

        // Test using value &= value.wrapping_sub(1) approach
        // Modified to also store indices in a vector for fair comparison
        let start = Instant::now();
        let value = complex_set.value();
        for _ in 0..100_000 {
            let mut v = value;
            while v != 0 {
                let mut indices = Vec::new();
                while v != 0 {
                    let i = value.trailing_zeros();
                    indices.push(i);
                    v &= v.wrapping_sub(1);
                }
                assert!(indices.len() > 0);
            }
        }
        let wrapping_sub_duration = start.elapsed();
        println!("ones_indices method took: {:?}", ones_indices_duration);
        println!("wrapping_sub approach took: {:?}", wrapping_sub_duration);

        // This test doesn't assert a specific winner, just compares and reports
        // Both approaches now build equivalent vectors of indices
    }
}
