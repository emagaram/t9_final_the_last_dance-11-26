import json
import os
import sys
from collections import Counter

LETTERS = "abcdefghijklmnopqrstuvwxyz'"


def letter_frequencies(word_freq: dict[str, float]) -> dict[str, float]:
    """Frequency of each letter, weighted by how often its word occurs, normalized to sum to 1."""
    counts = Counter()
    for word, freq in word_freq.items():
        for letter in word:
            counts[letter] += freq
    total = sum(counts.values())
    return {letter: counts[letter] / total for letter in LETTERS}


if __name__ == "__main__":
    # Defaults to the full dictionary in the repo root; pass another word_freq_*.json to override
    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(repo_root, "word_freq_full.json")

    with open(path) as f:
        word_freq = json.load(f)

    freqs = letter_frequencies(word_freq)
    print(f"Letter frequencies for {os.path.basename(path)} ({len(word_freq)} words)")
    for letter, freq in sorted(freqs.items(), key=lambda x: -x[1]):
        print(f"{letter}  {freq:.6f}")
