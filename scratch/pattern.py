from collections import Counter
from itertools import combinations

# ---------- helpers ---------------------------------------------------------
def flatten(groups: str) -> list[str]:
    """'ABC EFGH IJKL' -> ['A','B',...,'L']"""
    return [c for g in groups.split() for c in g]

def subgroups(full_letters: list[str]) -> list[set[str]]:
    """Remove one letter at every position."""
    return [set(full_letters[:i] + full_letters[i+1:])           # O(n²) but n ≤ 26 here
            for i in range(len(full_letters))]

def occurrences(pattern: str,
                all_subs: list[set[str]]) -> int:
    """How many sub-groups still contain *all* letters of pattern."""
    letters = set(pattern.replace(' ', ''))      # strip spaces such as 'AB EF'
    return sum(letters <= s for s in all_subs)   # subset test

# ---------- demo  -----------------------------------------------------------
groups   = "ABC EFGH IJKL"          # the “full” group
patterns = [                        # whatever patterns you care about
    "AB", "EF", "IJ",
    "ABC", "EFG", "EFGH",
    "AB EF", "IJ EF",
    "AB EFG", "IJ EFG",
    "AB EFGH", "ABC EFG",
    "IJK EFG", "ABC EFGH",
]

letters        = flatten(groups)           # 11 letters total
all_subgroups  = subgroups(letters)
n              = len(letters)

print(f"Total letters = {n}\n")
for pat in patterns:
    cnt = occurrences(pat, all_subgroups)
    print(f"{pat:<10s}:  counted = {cnt:2d}   expected = {n - len(pat.replace(' ', ''))}")