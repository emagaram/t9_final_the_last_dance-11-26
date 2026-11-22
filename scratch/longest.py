import json

def get_length_counts(json_file_path):
    """
    Get counts of words by their length from a JSON file with word-frequency pairs.

    Args:
        json_file_path (str): Path to the JSON file

    Returns:
        dict: mapping from word length to number of words of that length, or None on error
    """
    try:
        with open(json_file_path, 'r') as f:
            word_freq = json.load(f)

        length_counts = {}
        for word in word_freq:
            length = len(word)
            length_counts[length] = length_counts.get(length, 0) + 1

        return length_counts

    except FileNotFoundError:
        print(f"Error: The file {json_file_path} was not found.")
        return None
    except json.JSONDecodeError:
        print(f"Error: {json_file_path} is not a valid JSON file.")
        return None
def find_longest_word(json_file_path):
    """
    Find the longest word in a JSON file that contains word-frequency pairs.
    
    Args:
        json_file_path (str): Path to the JSON file
        
    Returns:
        tuple: (longest_word, length, frequency)
    """
    try:
        # Load the JSON file
        with open(json_file_path, 'r') as file:
            word_freq = json.load(file)
        
        # Find the longest word
        longest_word = ""
        for word in word_freq:
            if len(word) > len(longest_word):
                longest_word = word
        
        # Return the longest word, its length, and its frequency
        return (longest_word, len(longest_word), word_freq[longest_word])
    
    except FileNotFoundError:
        print(f"Error: The file {json_file_path} was not found.")
        return None
    except json.JSONDecodeError:
        print(f"Error: {json_file_path} is not a valid JSON file.")
        return None

if __name__ == "__main__":
    # Example usage
    result = find_longest_word("word_freq.json")
    if result:
        word, length, freq = result
        print(f"Longest word: {word}")
        print(f"Length: {length}")
        print(f"Frequency: {freq}")
    counts = get_length_counts("word_freq.json")
    
    total = sum(counts.values())
    for length, count in sorted(counts.items()):
        percentage = (count / total) * 100
        print(f"Length {length}: {count} words ({percentage:.2f}%)")