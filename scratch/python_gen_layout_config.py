def generate_combinations(X, Y_1, Y_2, min_item_value=1):
    """
    Generate all unique combinations of X positive integers that add up to a sum between Y_1 and Y_2 inclusive.
    Each combination is in non-decreasing order to ensure uniqueness.
    Each integer must be at least min_item_value.

    Args:
        X (int): Number of positive integers in each combination
        Y_1 (int): The minimum sum that each combination should add up to
        Y_2 (int): The maximum sum that each combination should add up to
        min_item_value (int): The minimum value that each integer must be

    Returns:
        list: List of tuples, each containing X positive integers with sum between Y_1 and Y_2
    """

    def backtrack(start, remaining_positions, current_sum, current_combination):
        # Base case: if we have X numbers in our combination
        if remaining_positions == 0:
            if Y_1 <= current_sum <= Y_2:
                result.append(tuple(current_combination))
            return

        # Calculate the minimum and maximum values for the current position
        min_val = max(
            start, min_item_value
        )  # Ensure non-decreasing order and minimum value
        # Maximum possible value while ensuring we can still have enough positions filled
        max_val = min(
            Y_2 - current_sum - (remaining_positions - 1) * min_item_value, Y_2
        )

        # Try each possible value for the current position
        for i in range(min_val, max_val + 1):
            current_combination.append(i)
            backtrack(i, remaining_positions - 1, current_sum + i, current_combination)
            current_combination.pop()

    result = []
    if X <= 0 or Y_1 <= 0 or Y_2 < Y_1:
        return result

    backtrack(min_item_value, X, 0, [])
    return result


if __name__ == "__main__":
    X = 3  # Number of integers in each combination
    Y_1 = 6  # Minimum sum
    Y_2 = 20  # Maximum sum
    min_item_value = 2  # Minimum value for each integer

    combinations = generate_combinations(X, Y_1, Y_2, min_item_value)
    for combo in combinations:
        print(combo)
    print(f"Total combinations: {len(combinations)}")
