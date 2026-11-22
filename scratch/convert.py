import json
import sys

def flatten_json(input_path, output_path):
    with open(input_path, 'r') as f:
        data = json.load(f)

    output = {
        "threshold": data["threshold"],
        "keys": [item for group in data["groups"] for item in group]
    }

    with open(output_path, 'w') as f:
        json.dump(output, f, indent=4)

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python flatten_json.py input.json output.json")
        sys.exit(1)

    flatten_json(sys.argv[1], sys.argv[2])