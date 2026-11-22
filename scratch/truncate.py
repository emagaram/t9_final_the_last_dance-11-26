import json
import os

def truncate_word_freq(input_file, output_sizes=(20000, 2000, 200)):
    """
    Truncate a word frequency dictionary to specific sizes and save as JSON files.
    
    Args:
        input_file (str): Path to the input JSON file containing word frequencies
        output_sizes (tuple): Sizes to truncate the dictionary to
    """
    # Load the original word frequency dictionary
    with open(input_file, 'r') as f:
        word_freq = json.load(f)
    
    print(f"Original dictionary size: {len(word_freq)} words")
    
    # Sort words by frequency (highest to lowest)
    sorted_words = sorted(word_freq.items(), key=lambda x: x[1], reverse=True)
    
    # Get the directory of the input file
    output_dir = os.path.dirname(input_file)
    
    # For each desired size, create a truncated dictionary and save it
    for size in output_sizes:
        # Make sure we don't try to create a dictionary larger than the original
        actual_size = min(size, len(sorted_words))
        
        # Create truncated dictionary
        truncated_dict = {word: freq for word, freq in sorted_words[:actual_size]}
        
        # Define output filename
        output_file = os.path.join(output_dir, f"word_freq_{actual_size}.json")
        
        # Save to JSON
        with open(output_file, 'w') as f:
            json.dump(truncated_dict, f, indent=4)
        
        print(f"Created {output_file} with {actual_size} words")

if __name__ == "__main__":
    # Get the path to the input file
    input_file = os.path.join(os.path.dirname(os.path.dirname(__file__)), "word_freq.json")
    
    # Truncate the dictionary
    truncate_word_freq(input_file, output_sizes=[30000])