import logging
import random
import string
import time
import traceback
import sys
import os
import datetime
from threading import Thread
from multiprocessing import Event, Process, cpu_count
from wordfreq import get_frequency_dict
import json


def monitor_for_termination(stop_event):
    """Monitor for the 'end' command in the console to terminate the processes."""
    while not stop_event.is_set():
        terminate_command = input()
        if terminate_command.lower() == "end":
            stop_event.set()


def write_end_time(base_path: str):
    """Write the current end time to a file."""
    end_time = datetime.datetime.now().strftime("%Y_%m_%d_%Hh_%Mm_%Ss")
    filename = os.path.join(base_path, "end_time.txt")
    with open(filename, "w") as file:
        file.write(end_time)


def read_best_score_from_file(base_path, score_file: str):
    # Initialize default values
    best_score = sys.float_info.max
    best_config = None
    path_name = os.path.join(base_path, "solutions", score_file)

    if os.path.exists(path_name):
        with open(path_name, "r") as file:
            lines = file.readlines()
            if len(lines) >= 2:
                best_score = float(lines[0].strip())
                best_config = eval(lines[1].strip())

    return best_score, best_config


def write_best_score_to_file(base_path, score: float, config: dict, score_file: str):
    # Ensure the Solutions folder exists
    path_name = os.path.join(base_path, "solutions")
    os.makedirs(path_name, exist_ok=True)
    # Path to the file within the Solutions folder
    full_path = os.path.join(path_name, score_file)

    # Write the score and config to the file
    with open(full_path, "w") as file:
        file.write(f"{score}\n{config}\n")


def solution_improvement_deadline(
    solution_improvement_deadline_base,
    solution_improvement_deadline_exp,
    all_time_score,
    current_best_score,
):
    closeness_boost = (
        3
        * solution_improvement_deadline_base
        * (
            (min(1, (current_best_score / all_time_score)))
            ** solution_improvement_deadline_exp
        )
    )
    # print(f"Giving {solution_improvement_deadline_base + closeness_boost}")
    return solution_improvement_deadline_base + closeness_boost


def crossover_uneven(
    parent1: dict[str, str], parent2: dict[str, str], letters, num_new_genes, kb_keys
):
    child: dict[str, str] = {}
    unused_letters = set(letters)
    unused_keys = set(kb_keys)
    for letter in random.sample(letters, k=len(letters) - num_new_genes):
        key = parent1[letter]
        child[letter] = key
        unused_letters.discard(letter)
        unused_keys.discard(key)
    for letter in unused_letters:
        child[letter] = parent2[letter] if len(unused_keys) == 0 else unused_keys.pop()
    return child


def generate_random_t10_uneven_reverse_config(LETTERS, KB_KEYS):
    t10_keys = {}
    for letter in LETTERS:
        t10_keys[letter] = random.choice(KB_KEYS)
    return t10_keys


def word_to_t10(word: str, t10_config: dict[str, str]):
    return "".join(t10_config[letter] for letter in word)


def reverse_dictionary(config):
    # Initialize an empty dictionary for the reversed mapping
    reverse_dict = {}

    # Iterate through each key-value pair in the original dictionary
    for key, value in config.items():
        # Iterate through each character in the value string
        for char in value:
            # Map each character to the original key
            reverse_dict[char] = reverse_dict.get(char, "") + key

    return reverse_dict


def print_t10_config_guess_percentage(config: dict[str, str], words_freq_list):
    # Initialize dictionary to store guess counts
    total_word_values = 0
    guess_counts = [0, 0, 0, 0, 0]
    t10_count = {}

    # Evaluate each word
    for word, value in words_freq_list:
        t10_word = word_to_t10(word, config)
        occurence = t10_count.get(t10_word, 0)
        t10_count[t10_word] = occurence + 1
        total_word_values += value
        if occurence < len(
            guess_counts
        ):  # Check if the word is within the first N guesses
            guess_counts[occurence] += value  # Increment the appropriate guess count

    # Print out the percentages for each guess
    for i, count in enumerate(guess_counts):
        print(
            f"{['First', 'Second', 'Third', 'Fourth', 'Fifth'][i]} guess percentage: {count*100/total_word_values:.3f}%"
        )


# Evaluate a given T10 configuration
def evaluate_t10_config_new(
    config: dict[str, str],
    w_freq_list: list[tuple[str, float]],
    best: float,
) -> float:
    score = 0
    index_count = {}

    for index, (word, value) in enumerate(w_freq_list):
        t10_word = word_to_t10(word, config)
        count = index_count.get(t10_word, 0)
        score += value * count
        index_count[t10_word] = count + 1
        if score > best:
            break
    return score


def get_number_unique_configs(configs: list[dict[str, str]]) -> int:
    seen = set()
    for config in configs:
        # Create a hashable representation of the config
        sorted_values_str = "".join(
            sorted("".join(sorted(value)) for value in config.values())
        )
        seen.add(sorted_values_str)
    return len(seen)


def mutate(config: dict[str, str], config_list: list[tuple[str, str]], kb_keys):
    # Random mutation - either swap two mappings or change a single mapping
    # if random.random() < 0.5:
    #     # Find keys that can be moved
    #     seen_chars = set()
    #     valid_keys_for_move = []

    #     for key, value in config_list:
    #         if any(char in seen_chars for char in value):
    #             valid_keys_for_move.append(key)
    #         seen_chars.update(value)

    #     # If we have valid keys, change one mapping
    #     if valid_keys_for_move:
    #         key_from = random.choice(valid_keys_for_move)
    #         key_to = random.choice(kb_keys)
    #         config[key_from] = key_to
    #     else:
    #         # Fallback to swap
    #         key_a, key_b = random.sample(config_list, 2)
    #         config[key_a[0]] = key_b[1]
    #         config[key_b[0]] = key_a[1]
    # else:
    # Swap two mappings
    key_a, key_b = random.sample(config_list, 2)
    # print(f"Swapping {key_a} with {key_b}")
    config[key_a[0]] = key_b[1]
    config[key_b[0]] = key_a[1]


def clean_dictionary(word_dict: dict[str, float], LETTERS):
    return {
        "".join(ch.lower() for ch in key if ch.isalpha() or ch == "'"): value
        for key, value in word_dict.items()
        if key and all(c.lower() in LETTERS for c in key)
    }


def append_config_if_needed(
    base_file_path: str,
    score: float,
    config: dict[str, str],
    configs_saved: list[int],
    max_configs: int,
    decent_score_threshold: float,
    filename: str,
):
    if score >= decent_score_threshold or configs_saved[0] >= max_configs:
        return

    solutions_path = os.path.join(base_file_path, "solutions")
    os.makedirs(solutions_path, exist_ok=True)
    full_filename = os.path.join(solutions_path, filename)

    configs_saved[0] += 1
    rev_config = reverse_dictionary(config)

    mode = "a" if os.path.exists(full_filename) else "w"
    with open(full_filename, mode) as file:
        file.write(f"{score}\n{rev_config}\n")


def get_index_from_percent(percent, total_freq, freq_list):
    reg_total_freq = percent * total_freq
    sum = 0
    reg_list_index = -1
    for index, (_, freq) in enumerate(freq_list):
        sum += freq
        if sum >= reg_total_freq:
            reg_list_index = index
            break
    if reg_list_index == -1:
        print(f"Miscalculating index. Percent: {percent}")
    return reg_list_index


def write_generations_completed(base_path: str, process_id: str, generations: int):
    directory_path = os.path.join(base_path, "performance")
    os.makedirs(directory_path, exist_ok=True)
    filename = os.path.join(directory_path, f"{process_id}_generations_completed.txt")
    with open(filename, "w") as file:
        file.write(f"{generations}")


def run_simulation(iteration_path: str, stop_event):

    NUM_KEYS = 10
    KB_KEYS = list((string.ascii_lowercase + ";")[:NUM_KEYS])
    LETTERS = list(string.ascii_lowercase + "'")
    POPULATION_SIZE = 60
    SOLUTION_IMPROVEMNT_DEADLINE_BASE = 300
    SOLUTION_IMPROVEMNT_DEADLINE_EXP = 5
    PROCESS_ID = os.getpid()
    ERRORS_LOG_FILENAME = f"{PROCESS_ID}_errors.log"
    SCORE_FILE = f"{PROCESS_ID}_best_{NUM_KEYS}_key_layout.txt"
    current_best_score = sys.float_info.max
    all_time_score = sys.float_info.max
    errors_path = os.path.join(iteration_path, "errors")
    os.makedirs(errors_path, exist_ok=True)
    log_file_path = os.path.join(errors_path, ERRORS_LOG_FILENAME)
    logging.basicConfig(
        filename=log_file_path,
        level=logging.ERROR,
        format="%(asctime)s:%(levelname)s:%(message)s",
    )

    current_best_config = {}
    # Load words frequency from a JSON file

    word_freq_file = "../word_freq_20000.json"
    try:
        with open(word_freq_file, "r") as file:
            freq_dict = clean_dictionary(json.load(file), LETTERS)
        freq_list = sorted(list(freq_dict.items()), key=lambda x: -x[1])
    except FileNotFoundError:
        logging.error(f"Could not find word frequency file: {word_freq_file}")
        print(f"Error: {word_freq_file} not found. Please make sure it exists.")
        stop_event.set()
        return

    population = [
        generate_random_t10_uneven_reverse_config(LETTERS, KB_KEYS)
        for _ in range(POPULATION_SIZE)
    ]
    scores_cache = {}
    generation_count = 1
    total_generation_count = 0
    solution_improvement_count = 0
    # start_time = datetime.datetime.now().strftime('%Y-%m-%d %H:%M:%S')  # Readable start time
    while not stop_event.is_set():
        if stop_event.is_set():
            break
        scored_population = []  # List to hold tuples of (score, config)

        # Called when no new improvements have been made in a while
        if solution_improvement_count > solution_improvement_deadline(
            SOLUTION_IMPROVEMNT_DEADLINE_BASE,
            SOLUTION_IMPROVEMNT_DEADLINE_EXP,
            all_time_score,
            current_best_score,
        ):
            total_generation_count += generation_count
            write_generations_completed(
                iteration_path, PROCESS_ID, total_generation_count
            )
            print(
                f"Generation died. Total generations: {generation_count}, Reg Score: {current_best_score}"
            )
            generation_count = 0
            solution_improvement_count = 0
            current_best_score = sys.float_info.max
            current_best_config = {}
            population = [
                generate_random_t10_uneven_reverse_config(LETTERS, KB_KEYS)
                for _ in range(POPULATION_SIZE)
            ]

        start_time = time.time()  # Start timing the generation

        def evaluate_population_member(config, config_list=None):
            try:
                if config_list is None:
                    config_list = sorted(config.items())
                config_key = tuple(config_list)

                if config_key not in scores_cache:
                    score = evaluate_t10_config_new(
                        config,
                        freq_list,
                        current_best_score,
                    )
                    scores_cache[config_key] = score
                else:
                    score = scores_cache[config_key]

                return score, config_list
            except Exception as e:
                logging.error(f"An unexpected error occurred: {e}")
                logging.error(
                    f"For config: {config}. Reversed: {reverse_dictionary(config)}"
                )
                exc_type, exc_value, exc_tb = sys.exc_info()
                line_number = traceback.extract_tb(exc_tb)[-1].lineno
                logging.error(f"Error occurred on line: {line_number}")
                logging.error("Traceback details:", traceback.format_exc())
                return sys.float_info.max, None

        for config in population:
            config_list = sorted(config.items())
            score, _ = evaluate_population_member(config, config_list)
            scored_population.append((score, config))

            if score < current_best_score:
                current_best_score = score
                current_best_config = config
                solution_improvement_count = 0
                if current_best_score < all_time_score:
                    all_time_score = current_best_score
                    write_best_score_to_file(
                        iteration_path,
                        all_time_score,
                        reverse_dictionary(current_best_config),
                        SCORE_FILE,
                    )
                    # print_t10_config_guess_percentage(current_best_config, freq_list)
                    print(all_time_score)
                    print()
        if len(scores_cache) == 1_000_000_000:
            scores_cache = {}
        # Sort the population based on the score (lower is better)
        scored_population.sort(key=lambda x: x[0])

        end_time = time.time()
        if generation_count % 100 == 0:
            print(
                f"Generation {generation_count} took {end_time - start_time:.4f} seconds, Uniqueness pct: {get_number_unique_configs(population)*100/len(population)}%"
            )
        generation_count += 1
        solution_improvement_count += 1

        # Selection: Top 1/2 automatically admitted, then new are generated from all
        next_population = [
            config for _, config in scored_population[: len(scored_population) // 2]
        ]

        potential_parents = [config for _, config in scored_population]
        random.shuffle(potential_parents)
        while len(next_population) < POPULATION_SIZE and len(potential_parents) >= 2:
            # Shuffle the potential parent pool every time to ensure randomness

            parent1 = potential_parents.pop()
            child = parent1.copy()
            mutate(child, config_list, KB_KEYS)
            next_population.append(child)

        population = next_population


if __name__ == "__main__":
    # Setup stop event
    stop_event = Event()

    # Start the termination monitor in a separate thread
    termination_thread = Thread(target=monitor_for_termination, args=(stop_event,))
    termination_thread.start()

    # Modify your main script logic here
    num_processes = int(1)
    iteration_id = datetime.datetime.now().strftime("%Y_%m_%d_%Hh_%Mm_%Ss")
    processes = []

    # Create a directory for output if it does not exist
    base_path = "normal_genetic"
    iteration_path = os.path.join(base_path, iteration_id)
    os.makedirs(iteration_path, exist_ok=True)

    for _ in range(num_processes):
        p = Process(target=run_simulation, args=(iteration_path, stop_event))
        p.start()
        processes.append(p)

    try:
        for p in processes:
            p.join()  # Wait for all processes to complete
    except KeyboardInterrupt:
        write_end_time(iteration_path)
    finally:
        # Ensure the input thread is also terminated
        termination_thread.join()
        # Finally, write the end time regardless of how the program exits
        write_end_time(iteration_path)
        print(f"End time written to {os.path.join(iteration_path, 'end_time.txt')}")
