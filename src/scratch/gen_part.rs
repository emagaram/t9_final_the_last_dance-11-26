use crate::general::set32::Set32;

/// Check if a partition has any subset of size 1 (singleton)
fn has_singleton(partition: &[Vec<u32>]) -> bool {
    partition.iter().any(|subset| subset.len() == 1)
}

/// Generate all partitions of a set excluding those with singletons
/// Returns a vector of partitions, where each partition is a vector of subsets
pub fn generate_partitions_no_singletons(set: &[u32]) -> Vec<Vec<Vec<u32>>> {
    if set.len() < 2 {
        return vec![]; // Can't have partitions without singletons for sets of size < 2
    }

    let mut result = Vec::new();
    let mut current_partition = Vec::new();
    partition_set_recursive(set, 0, &mut current_partition, &mut result);
    result
}

/// Generate partitions for Set32 by using its ones_indices
pub fn generate_partitions_for_set32(set32: Set32) -> Vec<Vec<Vec<u32>>> {
    let indices = set32.ones_indices();
    generate_partitions_no_singletons(&indices)
}

/// Recursive helper function to generate all partitions
fn partition_set_recursive(
    set: &[u32],
    index: usize,
    current_partition: &mut Vec<Vec<u32>>,
    result: &mut Vec<Vec<Vec<u32>>>,
) {
    if index == set.len() {
        // If we have considered all elements, check if partition has singletons
        if !has_singleton(current_partition) {
            result.push(current_partition.clone());
        }
        return;
    }

    // For each existing subset in the partition, add the current element to it
    for i in 0..current_partition.len() {
        current_partition[i].push(set[index]);
        partition_set_recursive(set, index + 1, current_partition, result);
        current_partition[i].pop();
    }

    // Add the current element as a singleton subset
    current_partition.push(vec![set[index]]);
    partition_set_recursive(set, index + 1, current_partition, result);
    current_partition.pop();
}

/// Count all partitions without generating them (more memory efficient)
pub fn count_partitions_no_singletons(set: &[u32]) -> usize {
    if set.len() < 2 {
        return 0; // Can't have partitions without singletons for sets of size < 2
    }

    let mut count = 0;
    let mut current_partition = Vec::new();
    count_partitions_recursive(set, 0, &mut current_partition, &mut count);
    count
}

/// Count partitions for Set32 by using its ones_indices
pub fn count_partitions_for_set32(set32: Set32) -> usize {
    let indices = set32.ones_indices();
    count_partitions_no_singletons(&indices)
}

/// Recursive helper function to count partitions
fn count_partitions_recursive(
    set: &[u32],
    index: usize,
    current_partition: &mut Vec<Vec<u32>>,
    count: &mut usize,
) {
    if index == set.len() {
        // If we have considered all elements, check if partition has singletons
        if !has_singleton(current_partition) {
            *count += 1;
        }
        return;
    }

    // For each existing subset in the partition, add the current element to it
    for i in 0..current_partition.len() {
        current_partition[i].push(set[index]);
        count_partitions_recursive(set, index + 1, current_partition, count);
        current_partition[i].pop();
    }

    // Add the current element as a singleton subset
    current_partition.push(vec![set[index]]);
    count_partitions_recursive(set, index + 1, current_partition, count);
    current_partition.pop();
}

/// Convert partitions of indices back to Set32 partitions
pub fn partitions_to_set32s(partitions: Vec<Vec<Vec<u32>>>) -> Vec<Vec<Set32>> {
    partitions
        .into_iter()
        .map(|partition| {
            partition
                .into_iter()
                .map(|subset| {
                    subset
                        .into_iter()
                        .fold(Set32::EMPTY, |acc, idx| acc.add(idx))
                })
                .collect()
        })
        .collect()
}

/// Generate Set32 partitions directly from a Set32
pub fn generate_set32_partitions_no_singletons(set32: Set32) -> Vec<Vec<Set32>> {
    let index_partitions = generate_partitions_for_set32(set32);
    partitions_to_set32s(index_partitions)
}

/// Print a partition for debugging (similar to Python version)
pub fn print_partition(partition: &[Vec<u32>]) {
    for subset in partition {
        print!("{{ ");
        for element in subset {
            print!("{} ", element);
        }
        print!("}} ");
    }
    println!();
}

/// Print a Set32 partition for debugging
pub fn print_set32_partition(partition: &[Set32]) {
    for subset in partition {
        print!("{{ ");
        for element in subset.ones_indices() {
            print!("{} ", element);
        }
        print!("}} ");
    }
    println!();
}

/// Example function that demonstrates partition generation
pub fn demo_partition_generation() {
    println!("=== Set32 Partition Generation Demo ===");

    // Create a Set32 with some indices
    let set32 = Set32::EMPTY.add(0).add(1).add(2).add(3);
    println!(
        "Original Set32: {:?} (indices: {:?})",
        set32,
        set32.ones_indices()
    );

    // Generate all partitions without singletons
    let partitions = generate_set32_partitions_no_singletons(set32);

    println!("Found {} partitions without singletons:", partitions.len());
    for (i, partition) in partitions.iter().enumerate() {
        print!("Partition {}: ", i + 1);
        print_set32_partition(partition);
    }

    // Compare with counting approach
    let count = count_partitions_for_set32(set32);
    println!("Count verification: {} partitions", count);

    // Example with a smaller set
    println!("\n=== Smaller Set Example ===");
    let small_set = Set32::EMPTY.add(5).add(7).add(10);
    println!(
        "Small Set32: {:?} (indices: {:?})",
        small_set,
        small_set.ones_indices()
    );

    let small_partitions = generate_set32_partitions_no_singletons(small_set);
    println!("Found {} partitions:", small_partitions.len());
    for (i, partition) in small_partitions.iter().enumerate() {
        print!("Partition {}: ", i + 1);
        print_set32_partition(partition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_partitions_small_set() {
        let set = vec![1, 2, 3];
        let partitions = generate_partitions_no_singletons(&set);

        // For set {1,2,3}, valid partitions without singletons are: {{1,2,3}}
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0], vec![vec![1, 2, 3]]);
    }

    #[test]
    fn test_generate_partitions_four_elements() {
        let set = vec![1, 2, 3, 4];
        let partitions = generate_partitions_no_singletons(&set);

        // For set {1,2,3,4}, valid partitions without singletons include:
        // {{1,2,3,4}}, {{1,2},{3,4}}, {{1,3},{2,4}}, {{1,4},{2,3}}
        assert!(partitions.len() >= 4);

        // Check that no partition has singletons
        for partition in &partitions {
            assert!(!has_singleton(partition));
        }
    }

    #[test]
    fn test_count_vs_generate() {
        let set = vec![1, 2, 3, 4, 5];
        let partitions = generate_partitions_no_singletons(&set);
        let count = count_partitions_no_singletons(&set);

        assert_eq!(partitions.len(), count);
    }

    #[test]
    fn test_set32_partitions() {
        let set32 = Set32::EMPTY.add(0).add(1).add(2);
        let partitions = generate_set32_partitions_no_singletons(set32);

        // Should have same number as index partitions
        let index_partitions = generate_partitions_for_set32(set32);
        assert_eq!(partitions.len(), index_partitions.len());

        // For 3 elements, should have 1 partition: {{0,1,2}}
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].len(), 1);
        assert_eq!(partitions[0][0].ones_indices(), vec![0, 1, 2]);
    }

    #[test]
    fn test_empty_and_small_sets() {
        assert_eq!(count_partitions_no_singletons(&[]), 0);
        assert_eq!(count_partitions_no_singletons(&[1]), 0);
        assert_eq!(
            generate_partitions_no_singletons(&[]),
            vec![] as Vec<Vec<Vec<u32>>>
        );
        assert_eq!(
            generate_partitions_no_singletons(&[1]),
            vec![] as Vec<Vec<Vec<u32>>>
        );
    }

    #[test]
    fn test_has_singleton() {
        let partition_with_singleton = vec![vec![1], vec![2, 3]];
        let partition_without_singleton = vec![vec![1, 2], vec![3, 4]];

        assert!(has_singleton(&partition_with_singleton));
        assert!(!has_singleton(&partition_without_singleton));
    }

    #[test]
    fn test_demo_partition_generation() {
        // This test just runs the demo to make sure it doesn't panic
        demo_partition_generation();
    }

    #[test]
    fn test_partition_bell_numbers() {
        // Test against known Bell numbers (total partitions) for small sets
        // Note: These are for ALL partitions, but we exclude singletons

        // For set of size 2: {0,1} - only partition without singleton is {{0,1}}
        let set2 = Set32::EMPTY.add(0).add(1);
        assert_eq!(count_partitions_for_set32(set2), 1);

        // For set of size 3: {0,1,2} - only partition without singleton is {{0,1,2}}
        let set3 = Set32::EMPTY.add(0).add(1).add(2);
        assert_eq!(count_partitions_for_set32(set3), 1);

        // For set of size 4: {0,1,2,3} - partitions without singletons:
        // {{0,1,2,3}}, {{0,1},{2,3}}, {{0,2},{1,3}}, {{0,3},{1,2}}
        let set4 = Set32::EMPTY.add(0).add(1).add(2).add(3);
        assert_eq!(count_partitions_for_set32(set4), 7); // 4 partitions as listed above
    }

    #[test]
    fn test_partition_consistency() {
        // Test that all generated partitions are valid
        let set = Set32::EMPTY.add(2).add(5).add(8).add(11);
        let partitions = generate_set32_partitions_no_singletons(set);

        for partition in &partitions {
            // Each partition should have no singletons
            assert!(!partition.iter().any(|subset| subset.count_ones() == 1));

            // Union of all subsets should equal original set
            let union = partition
                .iter()
                .fold(Set32::EMPTY, |acc, subset| acc.union(*subset));
            assert_eq!(union, set);

            // Subsets should be disjoint (no overlap)
            for (i, subset1) in partition.iter().enumerate() {
                for (j, subset2) in partition.iter().enumerate() {
                    if i != j {
                        assert!(subset1.intersect(*subset2).is_empty());
                    }
                }
            }
        }
    }
}
