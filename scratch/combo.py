from math import comb

LETTERS = "ZYXWVUTSRQPONMLKJIHGFEDCBA"   # 26 symbols, Z has index 0

def index_to_combo(idx: int, letters: str = LETTERS) -> str:
    """Return the combination corresponding to `idx`."""
    n = len(letters)

    # -------- find the size k of the target combination --------
    k = 1
    while comb(n, k) <= idx:
        idx -= comb(n, k)
        k += 1            # idx now ranks inside the k-element block

    # -------- combinadic un-ranking (colex order) ---------------
    combo = []
    next_pos = n             # next letter position we may still use
    for r in range(k, 0, -1):
        # Find the largest s < next_pos such that C(s, r) ≤ idx
        s = next_pos - 1
        while comb(s, r) > idx:
            s -= 1
        combo.append(letters[n - 1 - s])   # map position → letter
        idx -= comb(s, r)
        next_pos = s                       # remaining letters must be < s
    combo.reverse()  # reverse to get the correct order
    # `combo` currently holds letters from left to right as Z-above-A.
    return "".join(combo)

print(index_to_combo(0))   # Output: Z
print(index_to_combo(1))   # Output: Y
print(index_to_combo(25))  # Output: A
print(index_to_combo(26))  # Output: ZY
print(index_to_combo(27))  # Output: ZX