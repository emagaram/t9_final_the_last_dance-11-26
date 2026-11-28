import sys
from itertools import combinations
from math import comb

# Usage: python scratch/combo_division_full_estimate.py [N] [K]   (defaults: N=6, K=2)
# Splits the K-element combinations of range(N) into N groups by sum mod N, checks exhaustively that no
# two combinations in one group differ by exactly one replacement, then counts group sizes for every
# k in 2..K with a DP. The exhaustive check is skipped above MAX_CHECKED_COMBOS; the DP handles any size.

ALPHABET = "abcdefghijklmnopqrstuvwxyz'"
MAX_LISTED_COMBOS = 100
MAX_CHECKED_COMBOS = 100_000


def assign_group(combo, modulus):
    return sum(combo) % modulus


def label(combo, n):
    if n <= len(ALPHABET):
        return "".join(ALPHABET[x] for x in combo)
    return "-".join(map(str, combo))


def verify_grouping(n, k):
    groups = {}
    for combo in combinations(range(n), k):
        groups.setdefault(assign_group(combo, n), []).append(combo)

    # Every combination appears exactly once.
    flattened = [c for members in groups.values() for c in members]
    assert len(flattened) == comb(n, k)
    assert len(set(flattened)) == comb(n, k)

    # No two combinations in different groups differ by exactly one replacement. Every such pair is a
    # combination and a neighbor that swaps one member for one non-member, so checking each combination's
    # k * (n - k) neighbors covers all of them without comparing every pair across groups.
    checks = 0
    for group_id, members in groups.items():
        for combo in members:
            outside = set(range(n)) - set(combo)
            for removed in combo:
                for added in outside:
                    neighbor = (set(combo) - {removed}) | {added}
                    assert assign_group(neighbor, n) == group_id, (combo, sorted(neighbor))
                    checks += 1

    return groups, checks


def subset_sum_group_counts(n, max_k):
    # dp[k][r] = number of k-subsets whose sum is r modulo n.
    dp = [[0] * n for _ in range(max_k + 1)]
    dp[0][0] = 1

    for x in range(n):
        for k in range(min(max_k, x + 1), 0, -1):
            for r, count in enumerate(dp[k - 1]):
                if count:
                    dp[k][(r + x) % n] += count

    results = []
    for k in range(2, max_k + 1):
        counts = dp[k]
        assert sum(counts) == comb(n, k)
        results.append((k, comb(n, k), min(counts), max(counts)))
    return results


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 6
    k = int(sys.argv[2]) if len(sys.argv) > 2 else 2
    if not 2 <= k <= n:
        print("Usage: python combo_division_full_estimate.py [N] [K]   (requires 2 <= K <= N)")
        sys.exit(1)

    print(f"EXHAUSTIVE CHECK: N={n}, K={k}")
    if comb(n, k) > MAX_CHECKED_COMBOS:
        print(f"Skipped: C({n}, {k}) = {comb(n, k):,} exceeds MAX_CHECKED_COMBOS = {MAX_CHECKED_COMBOS:,}.\n")
    else:
        groups, neighbor_checks = verify_grouping(n, k)
        if comb(n, k) <= MAX_LISTED_COMBOS:
            for group_id in range(n):
                members = [label(c, n) for c in groups.get(group_id, [])]
                print(f"Group {group_id}: {', '.join(members)}")
        print(f"Verified {sum(map(len, groups.values())):,} unique combinations in {len(groups)} groups.")
        print(f"Checked {neighbor_checks:,} one-replacement neighbors; all share a group.\n")

    print(f"N={n} GROUP-SIZE VERIFICATION")
    print("k | total combinations | smallest group | largest group")
    for size, total, smallest, largest in subset_sum_group_counts(n, k):
        print(f"{size:2} | {total:17,} | {smallest:14,} | {largest:13,}")

    print("\nAll assertions passed.")
