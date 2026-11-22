/// Generate all unique combinations of X positive integers that add up to a sum between Y_1 and Y_2 inclusive.
/// Each combination is in non-decreasing order to ensure uniqueness.
/// Each integer must be at least min_item_value.
///
/// # Arguments
/// * `x` - Number of positive integers in each combination
/// * `y_1` - The minimum sum that each combination should add up to
/// * `y_2` - The maximum sum that each combination should add up to
/// * `min_item_value` - The minimum value that each integer must be
///
/// # Returns
/// Vector of vectors, each containing X positive integers with sum between Y_1 and Y_2
pub fn generate_layout_combinations(
    num_keys: usize,
    min_letter_count: usize,
    max_letter_count: usize,
    min_key_len: usize,
) -> Vec<Vec<usize>> {
    let mut result = Vec::new();

    if num_keys == 0 || min_letter_count <= 0 || max_letter_count < min_letter_count {
        return result;
    }

    fn backtrack(
        start: usize,
        remaining_positions: usize,
        current_sum: usize,
        current_combination: &mut Vec<usize>,
        result: &mut Vec<Vec<usize>>,
        y_1: usize,
        y_2: usize,
        min_item_value: usize,
    ) {
        // Base case: if we have X numbers in our combination
        if remaining_positions == 0 {
            if y_1 <= current_sum && current_sum <= y_2 {
                // Create a reversed version of the combination
                let mut reversed = current_combination.clone();
                reversed.reverse();
                result.push(reversed);
            }
            return;
        }

        // Calculate the minimum and maximum values for the current position
        let min_val = start.max(min_item_value); // Ensure non-decreasing order and minimum value

        // Maximum possible value while ensuring we can still have enough positions filled
        let max_val =
            (y_2 - current_sum - (remaining_positions as usize - 1) * min_item_value).min(y_2);

        // Try each possible value for the current position
        for i in min_val..=max_val {
            current_combination.push(i);
            backtrack(
                i,
                remaining_positions - 1,
                current_sum + i,
                current_combination,
                result,
                y_1,
                y_2,
                min_item_value,
            );
            current_combination.pop();
        }
    }

    let mut current_combination = Vec::new();
    backtrack(
        min_key_len,
        num_keys,
        0,
        &mut current_combination,
        &mut result,
        min_letter_count,
        max_letter_count,
        min_key_len,
    );

    result
}

pub fn calculate_min_max_letter_counts(
    target_num_keys: usize,
    current_num_keys: usize,
    num_letters: usize,
) -> (usize, usize) {
    let min_letter_count = current_num_keys * 2;
    let max_letter_count = num_letters - (target_num_keys - current_num_keys);

    // Ensure the sums are within the provided limits

    (min_letter_count, max_letter_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_layout_config_min_max_letter_counts() {
        let (min_letter_count, max_letter_count) = calculate_min_max_letter_counts(10, 7, 27);
        assert_eq!(min_letter_count, 14);
        assert_eq!(max_letter_count, 24);
    }

    //RUSTFLAGS="-Awarnings" cargo test --release test_gen_layout_config_basic_case -- --nocapture
    #[test]
    fn test_gen_layout_config_basic_case() {
        let combinations = generate_layout_combinations(3, 3, 9, 3);
        assert!(!combinations.is_empty());

        assert_eq!(combinations.len(), 1);
        assert_eq!(combinations[0], vec![3, 3, 3]);
    }

    // RUSTFLAGS="-Awarnings" cargo test --release gen_layout_config_impossible_config -- --nocapture
    #[test]
    fn gen_layout_config_impossible_config() {
        // Empty result cases
        assert_eq!(
            generate_layout_combinations(0, 5, 10, 1),
            Vec::<Vec<usize>>::new()
        );
        assert_eq!(
            generate_layout_combinations(3, 10, 5, 1),
            Vec::<Vec<usize>>::new()
        );
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_gen_layout_config_min_max_sums -- --nocapture
    #[test]
    fn test_gen_layout_config_min_max_sums() {
        let num_keys = 2;
        let min_key = 2;
        let max_letter_count = 6;
        let min_letter_count = 4;

        let combinations =
            generate_layout_combinations(num_keys, min_letter_count, max_letter_count, min_key);
        println!("Combinations: {:?}", combinations);
        for combo in &combinations {
            // All values should be at least 2
            for &val in combo {
                assert!(val >= min_key);
            }

            // Sum should be between 4 and 6
            let sum: usize = combo.iter().sum();
            assert!(sum <= max_letter_count);
            assert!(sum >= min_letter_count);
        }
    }
}
