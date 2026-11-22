use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{general::set32::Set32, globals::NUM_LETTERS, util::compare_letters};

/// Per-letter labels in alphabetical order: index 0 = `a` … 25 = `z`, 26 = `'`
pub type Labels = [u8; NUM_LETTERS];

/// Multi-letter keys decoded from a `PartitionId`, in canonical order
pub type PartitionKeys = SmallVec<[Set32; NUM_LETTERS / 2]>;

/// Order-independent ID of a partition of the letters into keys.
///
/// Each letter is labelled with the alphabet index of the first letter on its key
/// (letters on no multi-letter key label themselves), and the labels are read as a
/// base-27 number with `a` as the most significant digit. Two key lists get the same
/// ID exactly when they group the letters the same way, in any order.
///
/// A label is never larger than its own alphabet index, so `a`'s digit is always 0 and
/// every ID is below 27^26 (the largest, all singletons, is ~6.6e35 < u128::MAX).
#[derive(
    PartialEq, Eq, PartialOrd, Debug, Clone, Copy, Hash, Ord, Serialize, Deserialize, Decode, Encode,
)]
pub struct PartitionId(pub u128);

/// Alphabet index of a `Set32` letter bit (bits are reversed: `a` = 26, `'` = 0)
const fn alpha_index(bit: u32) -> usize {
    (NUM_LETTERS - 1) - bit as usize
}

impl PartitionId {
    /// 27^13. IDs are < 27^26, so `id / HALF` and `id % HALF` both fit in a u64.
    const HALF: u128 = (NUM_LETTERS as u128).pow(13);

    /// Every letter labelled with itself
    const IDENTITY_LABELS: Labels = {
        let mut labels = [0; NUM_LETTERS];
        let mut i = 0;
        while i < NUM_LETTERS {
            labels[i] = i as u8;
            i += 1;
        }

        labels
    };

    /// Every letter on its own key (the empty path)
    pub const SINGLETONS: PartitionId = Self::from_labels(&Self::IDENTITY_LABELS);

    pub fn value(&self) -> u128 {
        self.0
    }

    /// `keys` may be in any order; letters on no key are singletons.
    pub fn from_keys(keys: &[Set32]) -> PartitionId {
        let mut labels = Self::IDENTITY_LABELS;
        for &key in keys {
            // Highest bit = alphabetically first letter on the key
            let label = alpha_index(key.max_item().expect("Key cannot be empty")) as u8;
            for bit in key {
                labels[alpha_index(bit)] = label;
            }
        }
        Self::from_labels(&labels)
    }

    /// Same as `from_keys`, for groups given as letter bit indices (e.g. union-find output).
    pub fn from_letter_groups(groups: &[Vec<u8>]) -> PartitionId {
        let mut labels = Self::IDENTITY_LABELS;
        for group in groups {
            let label =
                alpha_index(*group.iter().max().expect("Group cannot be empty") as u32) as u8;
            for &bit in group {
                labels[alpha_index(bit as u32)] = label;
            }
        }
        Self::from_labels(&labels)
    }

    /// Reads `labels` as a base-27 number, `a` first. Each half is folded in u64 and
    /// combined once in u128: the low half has 13 digits (< 27^13 < 2^64), the high half
    /// has 14 but its first digit (`a`) is always 0, so it is also < 27^13.
    pub const fn from_labels(labels: &Labels) -> PartitionId {
        let base = NUM_LETTERS as u64;
        let (mut hi, mut lo) = (0u64, 0u64);
        let mut m = 0;
        while m < NUM_LETTERS {
            debug_assert!(labels[m] as usize <= m, "Label larger than its letter");
            if m < 14 {
                hi = hi * base + labels[m] as u64;
            } else {
                lo = lo * base + labels[m] as u64;
            }
            m += 1;
        }
        PartitionId(hi as u128 * Self::HALF + lo as u128)
    }

    /// Inverse of `from_labels`
    pub fn labels(&self) -> Labels {
        let base = NUM_LETTERS as u64;
        let (mut hi, mut lo) = ((self.0 / Self::HALF) as u64, (self.0 % Self::HALF) as u64);
        let mut labels = [0; NUM_LETTERS];
        // Digits come out least significant first: `'` (26) down to `a` (0)
        for m in (0..NUM_LETTERS).rev() {
            let half = if m < 14 { &mut hi } else { &mut lo };
            labels[m] = (*half % base) as u8;
            *half /= base;
        }
        labels
    }

    /// Multi-letter keys in canonical order (`compare_letters` descending), without
    /// heap allocation. Singletons are implicit and not returned.
    pub fn to_keys(&self) -> PartitionKeys {
        let mut keys = [Set32::EMPTY; NUM_LETTERS];
        for (m, &label) in self.labels().iter().enumerate() {
            keys[label as usize] = keys[label as usize].add(((NUM_LETTERS - 1) - m) as u32);
        }
        let mut result: PartitionKeys = keys.into_iter().filter(|k| k.count_ones() > 1).collect();
        result.sort_unstable_by(|a, b| compare_letters(*b, *a));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_set32s_vec, general::set32::Set32};
    use itertools::Itertools;

    #[test]
    fn test_singletons() {
        assert_eq!(PartitionId::from_keys(&[]), PartitionId::SINGLETONS);
        assert_eq!(
            PartitionId::SINGLETONS.labels(),
            PartitionId::IDENTITY_LABELS
        );
        assert_eq!(
            PartitionId::SINGLETONS.0,
            655956343554789600515162175472115225
        );
        assert!(PartitionId::SINGLETONS.0 < (NUM_LETTERS as u128).pow(26));
        assert!(PartitionId::SINGLETONS.to_keys().is_empty());
    }

    #[test]
    fn test_labels() {
        let id = PartitionId::from_keys(&create_set32s_vec!("abc,def,qtz"));
        assert_eq!(
            id.labels(),
            [
                0, 0, 0, 3, 3, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 16, 20, 21, 22,
                23, 24, 16, 26
            ]
        );
        assert_eq!(id.0, 2599564474624048284446163181452401);
        assert_eq!(PartitionId::from_labels(&id.labels()), id);
    }

    #[test]
    fn test_key_order_does_not_matter() {
        let keys = create_set32s_vec!("cd,ab,qxz,ef");
        let ids: Vec<PartitionId> = keys
            .iter()
            .copied()
            .permutations(keys.len())
            .map(|p| PartitionId::from_keys(&p))
            .collect();
        assert!(ids.iter().all(|&id| id == ids[0]));
        // Same letters, different grouping
        assert_ne!(
            ids[0],
            PartitionId::from_keys(&create_set32s_vec!("ac,bd,qxz,ef"))
        );
    }

    #[test]
    fn test_to_keys_canonical_order() {
        let canonical = create_set32s_vec!("qxz,ab,cd,ef")
            .into_iter()
            .sorted_by(|a, b| compare_letters(*b, *a))
            .collect_vec();
        let id = PartitionId::from_keys(&create_set32s_vec!("cd,ef,qxz,ab"));
        assert_eq!(id.to_keys().as_slice(), canonical.as_slice());
    }

    #[test]
    fn test_from_letter_groups_matches_from_keys() {
        let keys = create_set32s_vec!("abc,def,qtz");
        let groups: Vec<Vec<u8>> = keys.iter().map(|k| k.map(|b| b as u8).collect()).collect();
        assert_eq!(
            PartitionId::from_letter_groups(&groups),
            PartitionId::from_keys(&keys)
        );
    }

    #[test]
    fn test_every_full_layout_round_trips() {
        // Deterministic pseudo-random partitions of all 27 letters into up to 9 keys
        let mut state = 0x2545F4914F6CDD1Du64;
        for _ in 0..10_000 {
            let mut keys = [Set32::EMPTY; 9];
            for bit in 0..NUM_LETTERS as u32 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let k = (state % 9) as usize;
                keys[k] = keys[k].add(bit);
            }
            let multi: Vec<Set32> = keys.into_iter().filter(|k| k.count_ones() > 1).collect();
            let id = PartitionId::from_keys(&multi);
            let back = id.to_keys();
            assert_eq!(PartitionId::from_keys(&back), id);
            let mut sorted = multi.clone();
            sorted.sort_by(|a, b| compare_letters(*b, *a));
            assert_eq!(back.as_slice(), sorted.as_slice());
        }
    }

    /// The `KeyPath` ID functions `PartitionId` replaced, copied unchanged from before
    /// the refactor so the tests below can check the new type gives identical results.
    mod legacy {
        use crate::{
            general::set32::Set32,
            globals::{LETTER_MAP_ALPHABET, NUM_LETTERS},
            util::{compare_letters, num_arr_to_num, num_to_num_arr_target_digits},
        };
        use itertools::Itertools;

        pub fn letter_map_to_keys(letter_map: &[u128; NUM_LETTERS]) -> Vec<Set32> {
            let mut keys_map: [Set32; NUM_LETTERS] = [Set32::EMPTY; NUM_LETTERS];
            for (i, &value) in letter_map.iter().enumerate() {
                let letter = (NUM_LETTERS - 1) - i;
                keys_map[value as usize] = keys_map[value as usize].add(letter as u32);
            }
            keys_map
                .into_iter()
                .filter(|&set| set.count_ones() > 1)
                .sorted_by(|a, b| compare_letters(*b, *a))
                .collect()
        }
        pub fn keys_to_letter_map(keys: &Vec<Set32>) -> [u128; NUM_LETTERS] {
            let mut map = LETTER_MAP_ALPHABET.map(|v| v as u128);
            for key in keys {
                let ones = key.ones_indices();
                for i in ones.iter().rev() {
                    let map_idx = (NUM_LETTERS - 1) - *i as usize;
                    let map_value = (NUM_LETTERS - 1) - ones[ones.len() - 1] as usize;
                    map[map_idx] = map_value as u128;
                }
            }
            map
        }
        pub fn keys_to_u128(keys: &Vec<Set32>) -> u128 {
            let map = keys_to_letter_map(keys);
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
        /// Old `KeyPath::default().value`
        pub fn singletons() -> u128 {
            num_arr_to_num(&LETTER_MAP_ALPHABET.map(|v| v as u128), NUM_LETTERS as u128)
        }
    }

    /// Deterministic pseudo-random partial partitions: each letter is left off every key
    /// with probability ~1/3, otherwise put on one of 12 keys. Keys with one letter are
    /// dropped (singletons are implicit), so many letters end up as singletons.
    fn random_partial_partitions(count: usize) -> Vec<Vec<Set32>> {
        let mut state = 0x9E3779B97F4A7C15u64;
        (0..count)
            .map(|_| {
                let mut keys = [Set32::EMPTY; 12];
                for bit in 0..NUM_LETTERS as u32 {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    if state % 3 == 0 {
                        continue;
                    }
                    let k = ((state >> 8) % 12) as usize;
                    keys[k] = keys[k].add(bit);
                }
                keys.into_iter().filter(|k| k.count_ones() > 1).collect()
            })
            .collect()
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_matches_legacy_keypath -- --nocapture
    #[test]
    fn test_matches_legacy_keypath() {
        assert_eq!(PartitionId::SINGLETONS.0, legacy::singletons());

        for keys in random_partial_partitions(100_000) {
            let id = PartitionId::from_keys(&keys);

            // Encoding: same ID and same per-letter labels
            assert_eq!(id.0, legacy::keys_to_u128(&keys), "ID differs for {:?}", keys);
            assert_eq!(
                id.labels().map(|l| l as u128),
                legacy::keys_to_letter_map(&keys),
                "Labels differ for {:?}",
                keys
            );

            // Decoding: same keys in the same canonical order
            assert_eq!(id.to_keys().to_vec(), legacy::u128_to_keys_sorted(id.0));
            assert_eq!(
                id.to_keys().to_vec(),
                legacy::letter_map_to_keys(&legacy::keys_to_letter_map(&keys))
            );
            // The unsorted legacy decoder gives the same keys, just in label order
            let mut unsorted = legacy::u128_to_keys(id.0);
            unsorted.sort_by(|a, b| compare_letters(*b, *a));
            assert_eq!(id.to_keys().to_vec(), unsorted);
        }
    }

    #[test]
    fn test_legacy_key_order_does_not_matter() {
        // The old encoder was order-independent too, so both agree on every ordering
        let keys = create_set32s_vec!("cd,ab,qxz,ef");
        for p in keys.iter().copied().permutations(keys.len()) {
            assert_eq!(legacy::keys_to_u128(&p), legacy::keys_to_u128(&keys));
            assert_eq!(PartitionId::from_keys(&p).0, legacy::keys_to_u128(&p));
        }
    }

    #[test]
    fn test_bincode_matches_raw_u128() {
        // Node files store KeyPath::value; the newtype must not change the bytes
        let config = bincode::config::standard();
        for id in [
            PartitionId::SINGLETONS,
            PartitionId::from_keys(&create_set32s_vec!("abc,def,qtz")),
        ] {
            assert_eq!(
                bincode::encode_to_vec(id, config).unwrap(),
                bincode::encode_to_vec(id.0, config).unwrap()
            );
        }
    }
}
