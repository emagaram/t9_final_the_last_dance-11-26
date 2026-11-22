use std::collections::HashMap;

use crate::globals::NUM_LETTERS;

struct DSU {
    parent: Vec<u8>,
}

impl DSU {
    fn new(n: u8) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, x: u8) -> u8 {
        let xs = x as usize;
        if self.parent[xs] != x {
            let parent_x_usize = self.parent[xs] as usize;
            self.parent[xs] = self.find(self.parent[parent_x_usize]);
        }
        self.parent[xs]
    }
    fn union(&mut self, x: u8, y: u8) {
        let (rx, ry) = (self.find(x), self.find(y));
        if rx != ry {
            self.parent[ry as usize] = rx;
        }
    }
}

/// Merges linked letter pairs into connected groups (transitively), e.g.
/// `[ [(0,1), (1,2)] ]` -> `[ [0,1,2] ]`. Unpaired letters come back as singleton groups.
/// Group order is arbitrary; letters within each group are sorted descending.
pub fn connected_groups(pairs: &[(u8, u8)]) -> Vec<Vec<u8>> {
    let mut dsu = DSU::new(NUM_LETTERS as u8);
    for &(a, b) in pairs {
        dsu.union(a, b);
    }

    // Bucket every letter under its set's root
    let mut groups: HashMap<u8, Vec<u8>> = HashMap::new();
    for i in 0..NUM_LETTERS {
        let root = dsu.find(i as u8);
        groups.entry(root).or_default().push(i as u8);
    }
    // Sort each group in descending order
    for group in groups.values_mut() {
        group.sort_by(|a, b| b.cmp(a));
    }
    groups.into_values().collect()
}

#[cfg(test)]
mod tests {
    use gxhash::HashSet;

    use super::*;

    // RUSTFLAGS="-Awarnings" cargo test --release test_connected_groups -- --nocapture
    #[test]
    fn test_connected_groups() {
        // Test case 1: No connections
        let pairs: Vec<(u8, u8)> = vec![(1, 1), (0, 0)];
        let groups = connected_groups(&pairs);
        assert_eq!(groups.len(), NUM_LETTERS); // Each node is in its own group

        // Test case 2: Simple connections
        let pairs = vec![(0, 1), (2, 3), (4, 5), (1, 2)];
        let groups = connected_groups(&pairs);
        println!("Groups: {:?}", groups);
        // Find the group containing 0
        let connected_group = groups.iter().find(|g| g.contains(&0)).unwrap();

        // Check that 0,1,2,3 are in the same group
        assert!(connected_group.contains(&0));
        assert!(connected_group.contains(&1));
        assert!(connected_group.contains(&2));
        assert!(connected_group.contains(&3));

        // Test case 3: Disjoint groups
        let pairs = vec![(0, 1), (2, 3), (4, 5)];
        let groups = connected_groups(&pairs);
        println!("Groups: {:?}", groups);

        // Convert to sets for easier comparison
        let group_sets: Vec<HashSet<u8>> = groups
            .iter()
            .map(|g| g.iter().cloned().collect::<HashSet<u8>>())
            .collect();

        // Check that we have the expected connected components
        assert!(group_sets.iter().any(|s| s.contains(&0) && s.contains(&1)));
        assert!(group_sets.iter().any(|s| s.contains(&2) && s.contains(&3)));
        assert!(group_sets.iter().any(|s| s.contains(&4) && s.contains(&5)));
    }
    #[test]
    fn test_connected_groups_additional() {
        // Test case 4: Same node pair shouldn't break the union
        let pairs = vec![(2, 2), (3, 3)];
        let groups = connected_groups(&pairs);
        // Each node should still be in its own group except 2 and 3
        assert_eq!(groups.len(), NUM_LETTERS);

        // Test case 5: Complex connections with cycles
        let pairs = vec![(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6)];
        let groups = connected_groups(&pairs);

        // Find groups containing specific elements
        let group_0 = groups.iter().find(|g| g.contains(&0)).unwrap();
        let group_4 = groups.iter().find(|g| g.contains(&4)).unwrap();

        // Check that 0,1,2,3 are all in the same group
        assert!(
            group_0.contains(&0)
                && group_0.contains(&1)
                && group_0.contains(&2)
                && group_0.contains(&3)
        );

        // Check that 4,5,6 are in the same group
        assert!(group_4.contains(&4) && group_4.contains(&5) && group_4.contains(&6));

        // Test case 6: Overlapping pairs
        let pairs = vec![(0, 1), (1, 2), (2, 0), (3, 4), (4, 0)];
        let groups = connected_groups(&pairs);

        // All nodes 0-4 should be in the same group
        let connected_group = groups.iter().find(|g| g.contains(&0)).unwrap();
        for i in 0..=4 {
            assert!(connected_group.contains(&i));
        }

        // Test case 7: Multiple pairs connecting same nodes
        let pairs = vec![(0, 1), (0, 1), (0, 1), (2, 3), (2, 3)];
        let groups = connected_groups(&pairs);

        let group_sets: Vec<HashSet<u8>> = groups
            .iter()
            .map(|g| g.iter().cloned().collect::<HashSet<u8>>())
            .collect();

        assert!(group_sets.iter().any(|s| s.contains(&0) && s.contains(&1)));
        assert!(group_sets.iter().any(|s| s.contains(&2) && s.contains(&3)));
    }
}
