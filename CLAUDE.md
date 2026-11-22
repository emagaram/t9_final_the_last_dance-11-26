# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

An **exact** solver (global minimum, not a heuristic) for the optimal T9-style keyboard: assign 27 letters (a–z + `'`) to `NUM_KEYS` keys so that word collisions, weighted by frequency, are minimized. It is a Rust layer-by-layer DP over partial layouts. The solution tree is paged to disk, and pruning uses an upper-bound threshold plus admissible lower bounds.

When run locally, a smaller word dictionary of 20k words is used but the full solver runs the 300k word dictionary on an EC2 instance with ~256GB of RAM and ~4TB of stroage.

### Abstract problem → code mapping

- Objects = letters. Buckets = keys.
- A **conditional** is "groups G1..Gr each lie entirely within one key → add w". You get one per pair of same-length words (w_i more frequent than w_j):
  1. Pair up the letters at each position.
  2. Merge those pairs with union-find (`union_find::connected_groups`) to get the groups.
  3. If every group is co-located, the two words share a digit sequence, and the penalty is `freq(w_j)`.
- Built in `scores_hm_tree::gen_hm_vec` (O(n²) pairs per word-length group).
- Conditionals are bucketed by the **union of their letters** (`Set32`) and stored as one static trie per union: `hm_static_tree: HashMap<Set32, StaticTrie32>`.
  - Trie edges are the groups, in canonical order.
  - `get_intersection_score32` walks the trie. A group matches if it is a subset of the key that holds the group's lowest letter.
- Reference brute-force scorer: `evaluation/evaluate.rs` (`evaluate_arr*`). Each word adds `freq × (number of earlier, more-frequent words with the same digit sequence)`. Letters not on any key count as singleton keys. Tests compare the DP against this.

## Commands

```bash
cargo build --release
cargo run --release                     # runs the solver; all config is compile-time consts in src/settings.rs (no CLI args)
cargo run --profile dev-opt             # opt-level 3 + debug_assertions (enables invariant checks in hot loops)
cargo test --lib nodes::predecessor     # run one module's tests (or pass a single test name)
RUSTFLAGS="-Awarnings" cargo test --release <test_name> -- --nocapture   # convention used in test comments; changing RUSTFLAGS forces a full rebuild
cargo bench --bench micromap            # criterion benches: micromap, count_vs_count_ones, process_tasks, compression
```

Tests read data files relative to the repo root (`word_freq_*.json`, `test_data/`).

The suite is not uniformly green:
- `predecessor::test_get_full_paths_from_predecessor` currently fails.
- Some tests reference fixtures that don't exist (`word_freq_test.json`, `scores`, `test_data/sample_nodes.bin`).

Run targeted tests, not the whole suite.

## Core representations

- **`Set32`** (`general/set32.rs`) is a bitset of letters. Letter indices are **reversed**: `'`=0, z=1 … a=26, so `a` is the high bit.
  - Use `create_set32_str!("abc")`, `create_set32s_vec!("abc,de")` and `create_word!("the")` in tests.
  - Use `set32s_to_string` for debugging.
- **Path / `KeyPath`**: a partial layout, i.e. `Vec<Set32>` of keys with ≥2 letters each. Singletons are implicit and never stored.
  - Keys are kept in canonical order, sorted descending by `util::compare_letters` (popcount, then raw u32).
  - `key_set` = the union of all keys.
  - `value: u128` = an order-independent base-27 ID of the partition. It is the lookup key for predecessor scores.
- **Layout**: non-increasing key lengths, e.g. `[4,3,3,2]`. Its length is the number of multi-letter keys.
  - Layer n of the solver creates layouts of length n+1, stored under `L{n+1}/`.
- **Combo set**: the `Set32` of letters a node uses (= `key_set`). Node files are bucketed by combo set.
- **`StandardNode`** fields:
  - `path`
  - `score`
  - `score_by_letter_count: Vec<f32>`, where index k-2 = the penalty from conditionals involving exactly k distinct letters
  - `is_valid_set` / `is_valid` flags (written by the full-estimate pass)

## Solver data flow (`solver/solver.rs`)

1. **Startup** (`main.rs`):
   - If `RESET_DIR`, delete `ITERATION_ID/`.
   - Init tracing (`ITERATION_ID/solver.log`, `info.log`).
   - Load `State` from `ITERATION_ID/progress.bin`. If it doesn't exist, `create_root` writes the empty root node and the state starts at `layouts_to_eval = [[]]`.
   - Load the criteria trie from `hm_trie32_20000.bin`.
2. **For each layer, for each base layout: `set_children`** (`solver/set_children.rs`). Loop `key_len` over `2..=get_max_key_length` (which keeps layouts non-increasing) and build `full_layout = base + [key_len]`:
   1. Skip the layout if it is in `state.seen`. Otherwise delete any partial directory left by a crash.
   2. Find the predecessor layouts with `get_predecessor_layouts`: for each distinct run, shrink one key by 1, or drop it if it has 2 letters. All of their metadata must exist on disk; otherwise `break`.
   3. Generate every C(27, Σlayout) target combo set and send them in `READ_BATCH_SIZE` chunks to the **read workers** (`nodes/reader.rs::new_process_tasks`).
   4. The read workers `pread` the predecessor chunks for the target's minus-1 subsets (minus-2 when a 2-key was dropped). Nodes marked invalid are skipped.
      - Every predecessor fills `predecessors_map` (`u128 → score_by_letter_count`).
      - The **last** predecessor (the last key shrunk or dropped) supplies `base_nodes` to extend.
   5. The **calc workers** run on each base node:
      1. `get_full_path_from_predecessor` adds the missing letter to the last key, or appends a new 2-key. It rejects non-canonical results; this is the symmetry breaking that generates each partition once.
      2. `final_evaluate` scores the node.
      3. The node is dropped if the evaluation returns `None` or `score > threshold`.
      4. `scorecast.update_current_base_scores(key_set, score)` records the minimum score per letter set.
   6. One **writer thread** (`nodes/writer.rs`) appends a bincode chunk per combo set to the layout's `.bin`, then writes `metadata.bin` (combo set → byte range) by atomic rename.
      - If the layout produced no nodes, `break`; larger `key_len` would need those nodes as predecessors.
   7. Push the layout to `next_layouts` and `seen`, then call `state.save()`. This per-layout save is what makes runs resumable.
3. **`update_scorecast`**: load `scorecasts/scorecast_{layer}.bin` if it exists. Otherwise run `Scorecast::update(threshold)` and save the result.
4. **`set_full_estimate`** (`solver/set_full_estimate.rs`) goes over `next_layouts` in descending letter-sum order, so parents come first. It rewrites every node **in place** with `is_valid_set = true`. A node gets `is_valid` if either:
   - some valid same-layer parent (the same path with one more letter on some key) exists, or
   - `score + scorecast.get_score_target_num_keys(key_set) < threshold`.
5. Set `layouts_to_eval = next_layouts`, increment `layer`, and `state.save()`. Repeat while `layer < target_num_keys`.
   - The loop ends with a log line only; there is no "extract best layout" step.

### Scoring DP (`evaluation/final_evaluate/final_evaluate.rs`)

For path P with |P| letters:
- Sum `score_by_letter_count` over all "P minus one letter" sub-paths (`all_paths_minus_one`). Removing a letter from a 2-key yields the remaining path twice.
- Divide bucket k by `|P| − k`. A conditional on k letters appears in exactly that many sub-paths.
- Get the |P|-letter bucket fresh from `get_intersection_score32` on `hm_static_tree[key_set]`.

**Invariant**: every minus-one sub-path must be present in `predecessors_map`, or the node is dropped (`None`). So pruning a node implicitly prunes everything built on it. This is why `set_full_estimate` keeps nodes that have a valid parent.

### Pruning bounds

- **Upper bound**: `THRESHOLD_20K` in `settings.rs`, the best known layout score (from the genetic algorithm / `best_10.txt`) + `EPSILON`.
  - It must be ≥ the true optimum. If it is too low, layers come back empty (see the "FAILED" note in `settings.rs`).
- **Lower bound** (`scorecast/scorecast.rs`):
  - `pathkey_scores_by_layer[i]` = the minimum score per letter set over nodes with i+1 keys, padded with singleton letters.
  - `build_hm_par_from_base_scores` combines disjoint letter sets into "min cost to cover set S with k remaining keys". This is admissible because penalties are non-negative and additive.
  - Each rayon thread allocates a dense `vec![f32; 1<<27]` (512 MB), so memory scales with `SCORECAST_THREAD_NUMS`.

## On-disk layout

```
hm_trie32_20000.bin                         # prebuilt criteria trie (cwd-relative, gitignored)
scorecasts/scorecast_{layer}.bin            # cwd-relative, NOT under ITERATION_ID
{ITERATION_ID}/progress.bin                 # bincode State, atomic temp-file + rename
{ITERATION_ID}/L{n}/standard/{4_3_2}/{4_3_2}.bin   # concatenated bincode Vec<StandardNode> chunks
{ITERATION_ID}/L{n}/standard/{4_3_2}/metadata.bin  # HashMap<Set32 combo set, {start, len_bytes}>
```

Reads use `pread` (`nix`/`libc`), so the solver is Unix-only. Workers split as `cpu − 3` cores, `READ_CALC_RATIO : 1` readers to calc workers (`util::get_num_*_workers`).

## Gotchas

- **`RESET_DIR = true` wipes `ITERATION_ID/` on every start.** Set it to `false` to resume a run.
- **`scorecasts/` is not cleared by `RESET_DIR`.** A stale `scorecast_{layer}.bin` is silently loaded. Delete it when you change the threshold, dictionary or `NUM_KEYS`.
- **The trie path is hardcoded** in `solver.rs`, and `SOLVER_DICT` is unused. To change the dictionary, regenerate the trie and update the path. Nothing builds the trie automatically; call `gen_hm_vec(dict) → gen_hm_dyn_tree → gen_hm_static_tree32 → save_stream32(.., path)`.
- **`set_full_estimate` re-encodes node chunks at their original offset**, so a node's encoded size must not change. That's why the flags are two `bool`s rather than `Option<bool>`. Don't add variable-width fields that it mutates.
- **Path construction must keep canonical key order.** Trie edge lookup is a binary search and symmetry breaking relies on that order. The `#[cfg(debug_assertions)]` checks in `set_children` catch violations (run with `--profile dev-opt`).
- **Not on the solver path:**
  - `genetic/`: an offline GA used to find thresholds
  - `scorecast_old.rs`, `src/scratch/`, `scratch/*.py`: experiments and dictionary generation
  - `aws_save.rs`: S3 upload; `SAVE_TO_S3` is unused
  - `compression/`
- **Dictionaries** are `word_freq_*.json` files mapping word → frequency. They may only contain `a–z` and `'`; `ASCII_CODE_TO_INDEX` panics on any other character.
