import json
import string


def get_letters():
    return list(string.ascii_lowercase + "'")


def create_inaccuracy_freq_dict():
    with open("word_freq_20000.json", "r") as f:
        return json.load(f)


def create_inaccuracy_freq_list():
    return sorted(list(create_inaccuracy_freq_dict().items()), key=lambda x: -x[1])


def letter_sort_key(item):
    s = item[0]
    return [ord("z") + 1 if c == "'" else ord(c) for c in s]


def compress_array(arr):
    """
    Compress array by mapping unique values to consecutive integers.
    Time: O(n), Space: O(k) where k is unique values
    """
    mapping = {}
    next_val = 0
    result = []

    for num in arr:
        if num not in mapping:
            mapping[num] = next_val
            next_val += 1
        result.append(mapping[num])

    return result


# Test with your example
keyboard = [
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    11,
    11,
    12,
    13,
    14,
    15,
    16,
    17,
    17,
    17,
    22,
    22,
    22,
    25,
    26,
]
compressed = compress_array(keyboard)
print(f"Original:   {keyboard}")
print(f"Compressed: {compressed}")


# Alternative one-liner using dict.setdefault (same complexity)
def compress_oneliner(arr):
    mapping = {}
    return [mapping.setdefault(x, len(mapping)) for x in arr]


compressed2 = compress_oneliner(keyboard)
print(f"One-liner:  {compressed2}")

# Verify both give same result
print(f"Results match: {compressed == compressed2}")
