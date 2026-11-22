# T9 Final Project: Keyboard Optimizer

This project is a high-performance solver for optimizing T9-style keyboard layouts. It uses a combination of dynamic programming, parallel processing, score forecasting, intelligent pruning, and memory management techniques implemented in Rust to systematically discover the optimal arrangement of letters on a numeric keypad.

The primary goal is to maximize word prediction accuracy by minimizing collisions—ensuring that the most frequently used words have unique key sequences and don't conflict with other common words during predictive text input.

## Key Features

- **Resumability**: The solver's ability to run intermittently makes it cost-effective for Spot EC2 instances. To support this, the entire program state can be serialized, allowing it to be resumed at any point in the algorithm.
- **Efficient Evaluation**: A custom scoring system rapidly evaluates keyboard layout quality based on a word frequency dictionary, avoiding a full dictionary scan for each evaluation. While this requires an initial O(n²) pass over the dictionary, it results in an approximately 1000x speedup in scoring.
- **Dynamic Programming Solver**: The core of the project explores the space of possible keyboard layouts layer by layer, storing them in a tree to build an optimal solution from the ground up. Each partial layout (node) generates more complete layouts (children) for the next layer, with child scores calculated incrementally from a combination of scores from other keyboards.
- **Memory Paging**: The solver serializes chunks of the solution tree to disk, enabling it to manage terabytes of data while operating within RAM capacity limits.
- **Score Forecasting**: The `scorecast` module predicts the minimum possible final score of incomplete keyboard layouts, enabling more effective pruning.
- **Parallel Processing**: The solver reads and processes multiple tree chunks in parallel, leveraging the serialization format to efficiently utilize SSD bandwidth without over-throttling. This can be done with hundreds of threads since there are no bottlenecks (multiple SSDs or NVMe SSDs can be used to handle a large volume of IO requests).

## Project Structure

As a solo project, the code is a work in progress and is being actively refactored for clarity. It is organized into several key modules:

- [`src/solver`](src/solver): Implements the main dynamic programming algorithm for building the solution tree.
- [`src/evaluation`](src/evaluation): Contains the logic for scoring keyboard layouts.
- [`src/genetic`](src/genetic): Provides the genetic algorithm that is used to generate the cutoff score for pruning.
- [`src/nodes`](src/nodes): Defines the data structures for the solution tree nodes and handles their serialization to/from disk.
- [`src/scorecast`](src/scorecast): Implements the score forecasting logic.
- [`src/state.rs`](src/state.rs): Manages the overall state of the solver, including saving and loading progress.
- [`src/settings.rs`](src/settings.rs): Defines configuration settings for the application.
