use bincode::{Decode, Encode};

use crate::{
    general::{partition_id::PartitionId, set32::Set32},
    util::path_to_set32,
};

#[derive(Decode, Encode, Clone, PartialEq, Eq, Debug)]
pub struct KeyPath {
    pub value: PartitionId,
    pub keys: Vec<Set32>,
    pub key_set: Set32,
}

impl Default for KeyPath {
    fn default() -> Self {
        Self {
            value: PartitionId::SINGLETONS,
            keys: vec![],
            key_set: Set32::EMPTY,
        }
    }
}

impl KeyPath {
    pub fn from_set_path(path: Vec<Set32>) -> Self {
        Self {
            value: PartitionId::from_keys(&path),
            key_set: path_to_set32(&path),
            keys: path,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::{
        create_set32s_vec,
        general::{key_path::KeyPath, partition_id::PartitionId},
    };

    #[test]
    fn test_keypath_default() {
        let default = KeyPath::default();
        assert_eq!(default.keys, vec![]);
        assert_eq!(default.key_set, crate::general::set32::Set32::EMPTY);
        // The value should be the default mapping of the alphabet
        assert_eq!(default.value, PartitionId::SINGLETONS);
    }

    #[test]
    fn test_from_set_path_and_equality() {
        let keys = create_set32s_vec!("abc,def,qtz");
        let key_path = KeyPath::from_set_path(keys.clone());
        assert_eq!(key_path.keys, keys);
        assert_eq!(key_path.value, PartitionId::from_keys(&keys));
        assert_eq!(key_path, KeyPath::from_set_path(keys));
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
}
