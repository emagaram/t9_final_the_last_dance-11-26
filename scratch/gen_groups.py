import pickle
import json
from math import comb
import multiprocessing
import sys
from typing import List, Set, Tuple, Callable

from words import get_letters, create_inaccuracy_freq_list, letter_sort_key
from score import evaluate_config


def str_to_config(s: str) -> dict[str, str]:
    all_letters = get_letters()
    config = {}
    key_letter = s[0]
    for letter in all_letters:
        if letter in s:
            # The chosen letter maps to our input string
            config[letter] = key_letter
        else:
            config[letter] = letter
    return config


def my_scorer(
    batch: List[str],
    prev: Set[str],
    best: float = sys.float_info.max,
    process_id: int = -1,
) -> List[Tuple[str, float]]:
    """
    For each string in the batch:
    1. Check if every combination of s choose len(s)-1 is in the prev set
    2. Create a keyboard config where one key maps to the input string
    3. All other letters map to keys not in the input string
    4. Evaluate this config using the score.py functionality
    """
    results = []
    freq_list = create_inaccuracy_freq_list()[:20000]

    process_name = f"Process {process_id}" if process_id >= 0 else "Process"
    print(f"{process_name} starting with {len(batch)} items")

    evaluated_count = 0
    skipped_count = 0

    for s in batch:
        evaluate_conf = True
        for i in range(len(s)):
            subset = s[:i] + s[i + 1 :]
            if subset not in prev:
                evaluate_conf = False
                skipped_count += 1
                break
        if not evaluate_conf:
            continue

        # Only evaluate if all required subsets are present
        config = str_to_config(s)
        # Evaluate the config
        score = evaluate_config(config, freq_list, best)
        if score is None:
            skipped_count += 1
            continue

        evaluated_count += 1
        results.append((s, score))

    print(
        f"{process_name} finished: {evaluated_count} evaluated, {skipped_count} skipped"
    )
    return results


def gen_groups(
    P: int,
    best: float,
    parallel_fn: Callable[[List[str], Set[str]], List[Tuple[str, float]]],
) -> List[List[Tuple[str, float]]]:

    letters = get_letters()
    result: List[List[Tuple[str, float]]] = [[(c, 0.0) for c in letters]]

    # precompute index lookup for fast slicing
    idx = {c: i for i, c in enumerate(letters)}

    for i in range(1, len(letters)):
        prev_layer = result[i - 1]
        prev_strings = {s for s, _ in prev_layer}

        combos: List[str] = []
        for s, _ in prev_layer:
            for c in letters[idx[s[-1]] + 1 :]:
                combos.append(s + c)

        # Calculate how many combinations are theoretically possible at this layer
        total_theoretical = comb(len(letters), i + 1)
        percent_present = len(combos) / total_theoretical * 100
        percent_missing = 100 - percent_present
        print(
            f"Layer {i+1}: Generated {len(combos)} combos out of {total_theoretical} theoretical combinations"
        )
        print(
            f"        Present: {percent_present:.2f}%, Missing: {percent_missing:.2f}%"
        )

        chunks = [combos[j::P] for j in range(P)]

        with multiprocessing.Pool(P) as pool:
            asyncs = [
                pool.apply_async(parallel_fn, (chunk, prev_strings, best, j))
                for j, chunk in enumerate(chunks)
            ]
            parts = [a.get() for a in asyncs]
            for j in range(P):
                print(f"Process {j} for layer {i+1} completed")

        flat = [pair for part in parts for pair in part]
        result.append(sorted(flat, key=letter_sort_key))
    return result


# Finished in about 30min with 9 cores on a M1 Macbook Pro
if __name__ == "__main__":
    max_cores = max(1, multiprocessing.cpu_count() - 1)
    num_cores = max_cores
    num_keys = 10
    epsilon = 0.000001
    best = 0.014670087375483733 + epsilon  # in case we did find the best already
    print("Using", best, "as best")
    print("Using", epsilon, "as epsilon")
    print("Using", num_cores, "cores")
    print("Using", num_keys, "keys")
    print("Using", max_cores, "max cores")
    groups = gen_groups(P=min(num_cores, max_cores), best=best, parallel_fn=my_scorer)
    result = {
        "groups": groups,
        "input_best": best,
        "num_keys": num_keys,
    }

    with open(f"groups_{num_keys}.pkl", "wb") as f:
        pickle.dump(result, f)

    with open(f"groups_{num_keys}.json", "w") as f:
        json.dump(result, f, indent=4)
