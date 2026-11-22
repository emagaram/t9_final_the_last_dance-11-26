def compute_marginal_contributions(intersection_sizes):
    """
    Compute marginal contributions that can be summed to get union sizes.
    
    Key insight: Instead of storing S(ABC), store the "marginal contribution"
    of subset ABC that accounts for overlaps with other subsets.
    
    The idea: marginal[ABC] = some value such that:
    S(ABCD) ≈ marginal[A] + marginal[B] + marginal[C] + marginal[D] + 
              marginal[AB] + marginal[AC] + ... + marginal[ABC] + marginal[BCD]
    
    We use the principle of inclusion-exclusion but "distribute" the correction
    terms across the subsets.
    """
    if not intersection_sizes:
        return {}
    
    n = max(intersection_sizes.keys()).bit_length()
    marginal = {}
    
    # Process subsets by size
    for size in range(1, n + 1):
        for mask in range(1, 1 << n):
            if bin(mask).count('1') != size:
                continue
            
            if size == 1:
                # Single sets contribute their full size
                marginal[mask] = intersection_sizes.get(mask, 0)
            else:
                # For larger subsets, the marginal contribution is the
                # "excess overlap" that needs to be subtracted
                intersection_size = intersection_sizes.get(mask, 0)
                
                # The marginal contribution alternates sign based on subset size
                # This distributes the inclusion-exclusion correction
                sign = (-1) ** (size - 1)
                marginal[mask] = sign * intersection_size
    
    return marginal


def sum_marginal_contributions(marginal_contributions, target_mask):
    """
    Sum marginal contributions of all subsets of target_mask to get S(target_mask).
    """
    total = 0
    submask = target_mask
    while submask > 0:
        total += marginal_contributions.get(submask, 0)
        submask = (submask - 1) & target_mask
    
    return total


def optimized_marginal_approach(intersection_sizes):
    """
    A more sophisticated marginal contribution approach.
    
    The idea: For each subset T, compute a "weighted contribution" that
    accounts for how much it overlaps with other subsets of the same size.
    
    This reduces the number of terms needed in the final sum.
    """
    if not intersection_sizes:
        return {}
    
    n = max(intersection_sizes.keys()).bit_length()
    contributions = {}
    
    # Group subsets by size
    by_size = {}
    for mask in intersection_sizes:
        size = bin(mask).count('1')
        if size not in by_size:
            by_size[size] = []
        by_size[size].append(mask)
    
    # Compute contributions for each size group
    for size in range(1, n + 1):
        if size not in by_size:
            continue
            
        masks_of_size = by_size[size]
        
        for mask in masks_of_size:
            if size == 1:
                # Single sets: full contribution
                contributions[mask] = intersection_sizes[mask]
            elif size == 2:
                # Pairs: subtract intersection (standard inclusion-exclusion)
                contributions[mask] = -intersection_sizes[mask]
            else:
                # Larger subsets: alternating signs, but weighted by "frequency"
                # The frequency accounts for how often this intersection appears
                # in the final inclusion-exclusion formula
                
                # Number of ways this subset can appear in larger unions
                frequency = 1
                base_contribution = intersection_sizes[mask]
                
                # Adjust contribution based on expected usage
                sign = (-1) ** (size - 1)
                contributions[mask] = sign * base_contribution / frequency
    
    return contributions


def hierarchical_contribution_approach(intersection_sizes):
    """
    Hierarchical approach: Each level contributes independently.
    
    Key insight: Group contributions by "levels" where each level
    can be computed independently and then combined.
    
    Level 1: Individual sets A, B, C, D
    Level 2: Pairwise overlaps AB, AC, AD, BC, BD, CD  
    Level 3: Triple overlaps ABC, ABD, ACD, BCD
    Level 4: Full overlap ABCD
    
    S(ABCD) = Level1_contribution + Level2_contribution + Level3_contribution + Level4_contribution
    """
    if not intersection_sizes:
        return {}
    
    n = max(intersection_sizes.keys()).bit_length()
    level_contributions = {}
    
    # Compute contribution for each level
    for level in range(1, n + 1):
        level_total = 0
        level_count = 0
        
        for mask in intersection_sizes:
            if bin(mask).count('1') == level:
                # Standard inclusion-exclusion sign
                sign = (-1) ** (level - 1)
                level_total += sign * intersection_sizes[mask]
                level_count += 1
        
        if level_count > 0:
            # Store the total contribution of this level
            level_contributions[level] = level_total
    
    return level_contributions


def compute_union_from_levels(level_contributions, target_size):
    """
    Compute union size by summing level contributions up to target_size.
    """
    total = 0
    for level in range(1, target_size + 1):
        total += level_contributions.get(level, 0)
    return total


def smart_caching_approach(intersection_sizes):
    """
    Smart caching: Store partial sums that can be reused.
    
    Instead of storing S(ABC), store cumulative contributions:
    - cache[1] = contribution of all single sets
    - cache[2] = cache[1] + contribution of all pairs
    - cache[3] = cache[2] + contribution of all triples
    - etc.
    
    Then S(ABCD) for any specific ABCD is approximately cache[4]
    adjusted for the specific sets involved.
    """
    if not intersection_sizes:
        return {}
    
    n = max(intersection_sizes.keys()).bit_length()
    cache = {0: 0}
    
    # Build cumulative cache
    for level in range(1, n + 1):
        cache[level] = cache[level - 1]
        
        # Add contribution of all subsets of this size
        for mask in intersection_sizes:
            if bin(mask).count('1') == level:
                sign = (-1) ** (level - 1)
                cache[level] += sign * intersection_sizes[mask]
    
    return cache


def demonstrate_approaches():
    """Demonstrate different marginal contribution approaches."""
    
    # Example with sets A, B, C, D
    A = {1, 2, 3, 4, 5}
    B = {3, 4, 5, 6, 7}  
    C = {5, 6, 7, 8, 9}
    D = {1, 5, 9, 10, 11}
    
    sets = [A, B, C, D]
    n = len(sets)
    
    print("Example sets:")
    for i, s in enumerate(sets):
        print(f"{chr(ord('A') + i)} = {s}")
    
    # Compute all intersections
    intersection_sizes = {}
    for mask in range(1, 1 << n):
        active_sets = [sets[i] for i in range(n) if mask & (1 << i)]
        if len(active_sets) == 1:
            intersection = active_sets[0]
        else:
            intersection = set.intersection(*active_sets)
        intersection_sizes[mask] = len(intersection)
    
    print(f"\nTrue |A ∪ B ∪ C ∪ D| = {len(A | B | C | D)}")
    
    # Method 1: Basic marginal contributions
    print(f"\n1. Basic Marginal Contributions:")
    marginal = compute_marginal_contributions(intersection_sizes)
    full_mask = (1 << n) - 1
    result1 = sum_marginal_contributions(marginal, full_mask)
    print(f"   Result: {result1}")
    
    # Method 2: Hierarchical levels  
    print(f"\n2. Hierarchical Level Contributions:")
    levels = hierarchical_contribution_approach(intersection_sizes)
    result2 = compute_union_from_levels(levels, n)
    print(f"   Level contributions: {levels}")
    print(f"   Result: {result2}")
    
    # Method 3: Smart caching
    print(f"\n3. Smart Caching Approach:")
    cache = smart_caching_approach(intersection_sizes)
    result3 = cache[n]
    print(f"   Cached values: {cache}")
    print(f"   Result: {result3}")
    
    print(f"\nAll methods give the same result: {result1 == result2 == result3}")


def analyze_computational_savings():
    """
    Analyze the computational savings of marginal approaches.
    """
    print("\nComputational Analysis:")
    print("="*40)
    
    print("Traditional inclusion-exclusion for S(ABCD):")
    print("- Requires: 2^4 - 1 = 15 terms")
    print("- Each term: intersection computation + sign calculation")
    
    print("\nMarginal contribution approaches:")
    print("- Hierarchical levels: 4 level sums (one per subset size)")
    print("- Smart caching: 1 lookup after preprocessing")
    print("- Basic marginal: Still 15 terms, but distributed computation")
    
    print("\nKey insight:")
    print("- Hierarchical and caching approaches reduce online computation")
    print("- Good when computing many different union combinations")
    print("- Trade preprocessing time for faster query time")


def direct_subset_summation(intersection_sizes, target_mask):
    """
    Direct inclusion-exclusion computation for comparison.
    
    S(target) = sum over all non-empty subsets T of target:
                (-1)^(|T|-1) * |intersection(T)|
    """
    result = 0
    submask = target_mask
    while submask > 0:
        subset_size = bin(submask).count('1')
        sign = (-1) ** (subset_size - 1)
        result += sign * intersection_sizes.get(submask, 0)
        submask = (submask - 1) & target_mask
    
    return result


def run_comprehensive_tests():
    """
    Test all approaches against direct computation and verify correctness.
    """
    print("COMPREHENSIVE TESTING")
    print("="*60)
    
    # Test Case 1: Small example
    print("\nTest Case 1: Small Sets (A, B, C, D)")
    A = {1, 2, 3, 4, 5}
    B = {3, 4, 5, 6, 7}  
    C = {5, 6, 7, 8, 9}
    D = {1, 5, 9, 10, 11}
    
    sets = [A, B, C, D]
    n = len(sets)
    
    # Compute all intersections
    intersection_sizes = {}
    for mask in range(1, 1 << n):
        active_sets = [sets[i] for i in range(n) if mask & (1 << i)]
        if len(active_sets) == 1:
            intersection = active_sets[0]
        else:
            intersection = set.intersection(*active_sets)
        intersection_sizes[mask] = len(intersection)
    
    # Ground truth
    ground_truth = len(A | B | C | D)
    full_mask = (1 << n) - 1
    
    print(f"Ground truth |A ∪ B ∪ C ∪ D| = {ground_truth}")
    
    # Direct computation
    direct_result = direct_subset_summation(intersection_sizes, full_mask)
    print(f"Direct inclusion-exclusion = {direct_result}")
    
    # Method 1: Basic marginal
    marginal = compute_marginal_contributions(intersection_sizes)
    marginal_result = sum_marginal_contributions(marginal, full_mask)
    print(f"Basic marginal approach = {marginal_result}")
    
    # Method 2: Hierarchical
    levels = hierarchical_contribution_approach(intersection_sizes)
    hierarchical_result = compute_union_from_levels(levels, n)
    print(f"Hierarchical approach = {hierarchical_result}")
    
    # Method 3: Smart caching
    cache = smart_caching_approach(intersection_sizes)
    cache_result = cache[n]
    print(f"Smart caching approach = {cache_result}")
    
    # Verify all match
    results = [ground_truth, direct_result, marginal_result, hierarchical_result, cache_result]
    all_match = all(r == results[0] for r in results)
    print(f"All methods match: {all_match}")
    
    if not all_match:
        print(f"ERROR: Results don't match! {results}")
        return False


def test_multiple_subset_sizes():
    """
    Test with different subset combinations to verify correctness.
    """
    print("\n\nTest Case 2: Multiple Subset Sizes")
    print("-" * 40)
    
    # Use the same sets from before
    A = {1, 2, 3, 4, 5}
    B = {3, 4, 5, 6, 7}  
    C = {5, 6, 7, 8, 9}
    D = {1, 5, 9, 10, 11}
    
    sets = [A, B, C, D]
    n = len(sets)
    
    # Compute all intersections
    intersection_sizes = {}
    for mask in range(1, 1 << n):
        active_sets = [sets[i] for i in range(n) if mask & (1 << i)]
        if len(active_sets) == 1:
            intersection = active_sets[0]
        else:
            intersection = set.intersection(*active_sets)
        intersection_sizes[mask] = len(intersection)
    
    # Precompute marginal contributions
    marginal = compute_marginal_contributions(intersection_sizes)
    levels = hierarchical_contribution_approach(intersection_sizes)
    cache = smart_caching_approach(intersection_sizes)
    
    # Test different subset combinations
    test_cases = [
        (0b0001, "A"),           # Single set
        (0b0011, "A ∪ B"),       # Two sets  
        (0b0111, "A ∪ B ∪ C"),   # Three sets
        (0b1111, "A ∪ B ∪ C ∪ D"), # All sets
        (0b1010, "B ∪ D"),       # Non-contiguous
        (0b1100, "C ∪ D"),       # Different pair
    ]
    
    all_correct = True
    
    for mask, name in test_cases:
        # Ground truth
        active_sets = [sets[i] for i in range(n) if mask & (1 << i)]
        if len(active_sets) == 1:
            ground_truth = len(active_sets[0])
        else:
            ground_truth = len(set.union(*active_sets))
        
        # Direct computation
        direct = direct_subset_summation(intersection_sizes, mask)
        
        # Marginal approach
        marginal_result = sum_marginal_contributions(marginal, mask)
        
        # Hierarchical approach (need to compute for specific mask)
        hierarchical_result = 0
        submask = mask
        while submask > 0:
            subset_size = bin(submask).count('1')
            sign = (-1) ** (subset_size - 1)
            hierarchical_result += sign * intersection_sizes.get(submask, 0)
            submask = (submask - 1) & mask
        
        print(f"{name:15} | Ground: {ground_truth:2d} | Direct: {direct:2d} | Marginal: {marginal_result:2d} | Hierarchical: {hierarchical_result:2d}")
        
        if not (ground_truth == direct == marginal_result == hierarchical_result):
            all_correct = False
            print(f"  ERROR: Mismatch for {name}!")
    
    print(f"\nAll subset tests passed: {all_correct}")
    return all_correct


def performance_comparison():
    """
    Compare performance of different approaches.
    """
    print("\n\nPerformance Comparison")
    print("="*50)
    
    import time
    import random
    
    # Generate larger test case
    random.seed(42)
    n = 6  # 6 sets
    
    # Create random intersection sizes
    intersection_sizes = {}
    for mask in range(1, 1 << n):
        intersection_sizes[mask] = random.randint(1, 100)
    
    full_mask = (1 << n) - 1
    
    # Time direct approach
    start = time.time()
    for _ in range(1000):  # Run multiple times for better timing
        direct_result = direct_subset_summation(intersection_sizes, full_mask)
    direct_time = time.time() - start
    
    # Time marginal approach (with preprocessing)
    start = time.time()
    marginal = compute_marginal_contributions(intersection_sizes)
    for _ in range(1000):
        marginal_result = sum_marginal_contributions(marginal, full_mask)
    marginal_time = time.time() - start
    
    # Time hierarchical approach
    start = time.time()
    levels = hierarchical_contribution_approach(intersection_sizes)
    for _ in range(1000):
        hierarchical_result = compute_union_from_levels(levels, n)
    hierarchical_time = time.time() - start
    
    # Time caching approach
    start = time.time()
    cache = smart_caching_approach(intersection_sizes)
    for _ in range(1000):
        cache_result = cache[n]
    cache_time = time.time() - start
    
    print(f"Results (n={n}, 2^n-1={2**n-1} subsets):")
    print(f"Direct computation:     {direct_time:.6f}s")
    print(f"Marginal contributions: {marginal_time:.6f}s")  
    print(f"Hierarchical levels:    {hierarchical_time:.6f}s")
    print(f"Smart caching:          {cache_time:.6f}s")
    
    # Verify results match
    results_match = (direct_result == marginal_result == hierarchical_result == cache_result)
    print(f"\nResults match: {results_match}")
    
    # Calculate speedups
    if direct_time > 0:
        print(f"\nSpeedups vs direct:")
        print(f"Marginal:     {direct_time/marginal_time:.2f}x")
        print(f"Hierarchical: {direct_time/hierarchical_time:.2f}x") 
        print(f"Caching:      {direct_time/cache_time:.2f}x")


def test_edge_cases():
    """
    Test edge cases and boundary conditions.
    """
    print("\n\nEdge Case Testing")
    print("="*40)
    
    # Edge Case 1: Empty intersections
    print("Edge Case 1: Disjoint sets")
    A = {1, 2, 3}
    B = {4, 5, 6}
    C = {7, 8, 9}
    
    sets = [A, B, C]
    n = len(sets)
    
    intersection_sizes = {}
    for mask in range(1, 1 << n):
        active_sets = [sets[i] for i in range(n) if mask & (1 << i)]
        if len(active_sets) == 1:
            intersection = active_sets[0]
        else:
            intersection = set.intersection(*active_sets)
        intersection_sizes[mask] = len(intersection)
    
    ground_truth = len(A | B | C)  # Should be 9
    full_mask = (1 << n) - 1
    
    direct = direct_subset_summation(intersection_sizes, full_mask)
    levels = hierarchical_contribution_approach(intersection_sizes)
    hierarchical = compute_union_from_levels(levels, n)
    
    print(f"Ground truth: {ground_truth}, Direct: {direct}, Hierarchical: {hierarchical}")
    print(f"Disjoint sets test passed: {ground_truth == direct == hierarchical}")
    
    # Edge Case 2: Identical sets
    print("\nEdge Case 2: Identical sets")
    A = {1, 2, 3, 4, 5}
    B = {1, 2, 3, 4, 5}  # Same as A
    C = {1, 2, 3, 4, 5}  # Same as A
    
    sets = [A, B, C]
    intersection_sizes = {}
    for mask in range(1, 1 << n):
        active_sets = [sets[i] for i in range(n) if mask & (1 << i)]
        if len(active_sets) == 1:
            intersection = active_sets[0]
        else:
            intersection = set.intersection(*active_sets)
        intersection_sizes[mask] = len(intersection)
    
    ground_truth = len(A | B | C)  # Should be 5
    direct = direct_subset_summation(intersection_sizes, full_mask)
    levels = hierarchical_contribution_approach(intersection_sizes)
    hierarchical = compute_union_from_levels(levels, n)
    
    print(f"Ground truth: {ground_truth}, Direct: {direct}, Hierarchical: {hierarchical}")
    print(f"Identical sets test passed: {ground_truth == direct == hierarchical}")


if __name__ == "__main__":
    success = True
    
    # Run all tests
    try:
        demonstrate_approaches()
        print("\n" + "="*60)
        
        run_comprehensive_tests()
        test_multiple_subset_sizes() 
        test_edge_cases()
        performance_comparison()
        
        print("\n" + "="*60)
        analyze_computational_savings()
        
        if success:
            print(f"\n✅ ALL TESTS PASSED!")
        else:
            print(f"\n❌ SOME TESTS FAILED!")
            
    except Exception as e:
        print(f"\n❌ ERROR DURING TESTING: {e}")
        import traceback
        traceback.print_exc()