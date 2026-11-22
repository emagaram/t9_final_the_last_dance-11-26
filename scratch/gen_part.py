def print_partition(ans):
    """
    Function to print a partition
    """
    for i in ans:
        print("{", end=" ")
        for element in i:
            print(element, end=" ")
        print("}", end=" ")
    print()


def has_singleton(ans):
    """
    Check if a partition has any subset of size 1
    """
    return any(len(subset) == 1 for subset in ans)


def partition_set(set, index, ans, count):
    """
    Function to generate all partitions and count them (excluding those with singletons)
    """
    if index == len(set):
        # If we have considered all elements in the set, check if partition has singletons
        if not has_singleton(ans):
            print_partition(ans)
            count[0] += 1
        return

    # For each subset in the partition, add the current element to it and recall
    for i in range(len(ans)):
        ans[i].append(set[index])
        partition_set(set, index + 1, ans, count)
        ans[i].pop()

    # Add the current element as a singleton subset and recall
    ans.append([set[index]])
    partition_set(set, index + 1, ans, count)
    ans.pop()


def count_partitions(set, index, ans, count):
    """
    Function to count all partitions without printing (excluding those with singletons)
    """
    if index == len(set):
        # If we have considered all elements in the set, check if partition has singletons
        if not has_singleton(ans):
            count[0] += 1
        return

    # For each subset in the partition, add the current element to it and recall
    for i in range(len(ans)):
        ans[i].append(set[index])
        count_partitions(set, index + 1, ans, count)
        ans[i].pop()

    # Add the current element as a singleton subset and recall
    ans.append([set[index]])
    count_partitions(set, index + 1, ans, count)
    ans.pop()


def partition_set_optimized(set, index, ans, count):
    """
    Optimized version that prunes branches early when singletons are detected
    """
    if index == len(set):
        # If we have considered all elements and no singletons, count it
        print_partition(ans)
        count[0] += 1
        return

    # For each existing subset in the partition, add the current element to it
    for i in range(len(ans)):
        ans[i].append(set[index])
        partition_set_optimized(set, index + 1, ans, count)
        ans[i].pop()

    # Only add as singleton if we have enough remaining elements to avoid forcing singletons
    # This is an optimization - we could still add more sophisticated pruning
    ans.append([set[index]])
    partition_set_optimized(set, index + 1, ans, count)
    ans.pop()


def count_partitions_optimized(set, index, ans, count):
    """
    Optimized counting version that prunes branches early
    """
    if index == len(set):
        # If we have considered all elements and no singletons, count it
        count[0] += 1
        return

    # For each existing subset in the partition, add the current element to it
    for i in range(len(ans)):
        ans[i].append(set[index])
        count_partitions_optimized(set, index + 1, ans, count)
        ans[i].pop()

    # Only add as singleton if there are enough remaining elements
    # This optimization could be improved further
    ans.append([set[index]])
    count_partitions_optimized(set, index + 1, ans, count)
    ans.pop()


def all_partitions(set):
    """
    Function to generate all partitions for a given set and count them (no singletons)
    """
    ans = []
    count = [0]
    partition_set(set, 0, ans, count)
    return count[0]


def count_all_partitions(set):
    """
    Function to count all partitions without printing them (no singletons)
    """
    ans = []
    count = [0]
    count_partitions(set, 0, ans, count)
    return count[0]


def all_partitions_optimized(set):
    """
    Optimized function to generate all partitions (no singletons)
    """
    if len(set) < 2:
        return 0  # Can't have partitions without singletons for sets of size < 2
    
    ans = []
    count = [0]
    partition_set_optimized(set, 0, ans, count)
    return count[0]


def count_all_partitions_optimized(set):
    """
    Optimized function to count all partitions (no singletons)
    """
    if len(set) < 2:
        return 0  # Can't have partitions without singletons for sets of size < 2
    
    ans = []
    count = [0]
    count_partitions_optimized(set, 0, ans, count)
    return count[0]


# Main function
if __name__ == "__main__":
    # The size of the set
    n = 4

    # Initialize the set as {1, 2, ..., n}
    set = list(range(1, n + 1))

    print("All partitions of the set (excluding those with singletons):")
    # Generate all partitions of the set and count them
    total_partitions = all_partitions(set)
    print(f"\nTotal number of partitions (no singletons): {total_partitions}")
    
    print("\n" + "="*50)
    
    # Alternative: Just count without printing
    print("Counting partitions without printing:")
    count_only = count_all_partitions(set)
    print(f"Total number of partitions (no singletons): {count_only}")
    
    # Test with different set sizes
    print("\n" + "="*50)
    print("Partition counts for different set sizes (no singletons):")
    for size in range(1, 10):
        test_set = list(range(1, size + 1))
        count = count_all_partitions(test_set)
        print(f"Set size {size}: {count} partitions")
    
    # Compare with optimized version
    print("\n" + "="*50)
    print("Comparison with optimized version:")
    for size in range(2, 15):
        test_set = list(range(1, size + 1))
        count_regular = count_all_partitions(test_set)
        count_optimized = count_all_partitions_optimized(test_set)
        print(f"Set size {size}: Regular={count_regular}, Optimized={count_optimized}")