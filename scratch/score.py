import os
import string
import sys
from typing import Optional
from wordfreq import get_frequency_dict

from words import create_inaccuracy_freq_list

def evaluate_config(
    config: dict[str, str],
    w_freq_list: list[tuple[str, float]],
    best: float,
) -> Optional[float]:
    score = 0
    index_count = {}
    for word, freq in w_freq_list:
        t10_word = word_to_t10(word, config)
        count = index_count.get(t10_word, 0)
        score += freq * count
        index_count[t10_word] = count + 1
        if score > best:
            return None
    return score

def word_to_t10(word: str, config: dict[str, str]):
    t10_word = []
    for letter in word:
        t10_word.append(config[letter])
    return "".join(t10_word)


if __name__ == "__main__":
    # Example usage
    # ab, ....
    example_config = {
        'a': 'q', 'b': 'q', 'c': 'e', 'd': 'r', 'e': 't',
        'f': 'y', 'g': 'u', 'h': 'i', 'i': 'o', 'j': 'p',
        'k': 'a', 'l': 's', 'm': 'd', 'n': 'f', 'o': 'g',
        'p': 'h', 'q': 'j', 'r': 'k', 's': 'l', 't': 'z',
        'u': 'x', 'v': 'c', 'w': 'v', 'x': 'b', 'y': 'n',
        'z': 'm', "'": ";"
    }
    fl = create_inaccuracy_freq_list()
    score = evaluate_config(example_config, fl, 10)
    print(f"Score for the given keyboard configuration: {score}")
