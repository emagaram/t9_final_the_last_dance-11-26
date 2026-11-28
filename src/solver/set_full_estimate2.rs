use std::{
    cmp::Reverse,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use bincode::config;
use tracing::{info, span, Level};

use crate::{
    general::set32::Set32,
    scorecast::scorecast::Scorecast,
    settings::READ_BATCH_SIZE,
    solver::{
        pending_marks::{LayerChunks, MarksStats, PendingMarks},
        pipeline::run_pipeline,
    },
};

/// Same result as `set_full_estimate`: every node gets `is_valid_set`, and `is_valid` if the scorecast allows it or
/// a valid same-layer parent (one more letter on a key) needs it.
///
/// Instead of reading its parents, each valid node marks its children (one letter fewer). Layouts run parents
/// first, so a chunk's marks are all in before it's evaluated. Marks wait in memory until then, unless the ones
/// that can be flushed go over `cap_bytes`.
pub fn set_full_estimate2(
    scorecast: &Scorecast,
    layouts_to_eval: &Vec<Vec<usize>>,
    directory: &str,
    threshold: f32,
    cap_bytes: usize,
) -> MarksStats {
    let span_set_full_estimate = span!(
        Level::INFO,
        "set_full_estimate2",
        layouts_to_eval = ?layouts_to_eval
    );
    let _enter_full_estimate = span_set_full_estimate.enter();
    let mut layouts = layouts_to_eval.clone();
    layouts.sort_by_key(|layout| Reverse(layout.iter().sum::<usize>()));
    let chunks = LayerChunks::load(directory, layouts);
    let mut marks = PendingMarks::new(&chunks, cap_bytes);

    for current in 0..chunks.layouts.len() {
        let start = Instant::now();
        marks.start_layout(current);
        let file = chunks.open(current);
        let mut combo_sets: Vec<Set32> = chunks.metadata[current].items.keys().copied().collect();
        combo_sets.sort_unstable();
        let nodes_seen = AtomicUsize::new(0);
        let nodes_valid = AtomicUsize::new(0);

        let stats = run_pipeline(
            combo_sets.chunks(READ_BATCH_SIZE),
            |batch| {
                batch
                    .iter()
                    .map(|&combo_set| (combo_set, chunks.read(&file, current, combo_set)))
                    .collect::<Vec<_>>()
            },
            |batch, write_sender| {
                marks.run_task(|| {
                    for (combo_set, mut nodes) in batch {
                        let marked = marks.take(combo_set);
                        debug_assert_eq!(
                            nodes
                                .iter()
                                .filter(|node| marked.contains(&node.path.value))
                                .count(),
                            marked.len(),
                            "Marked node missing from its chunk"
                        );
                        for node in &mut nodes {
                            node.is_valid_set = true;
                            // Already valid if a parent marked it, here or in an earlier flush
                            node.is_valid = node.is_valid
                                || marked.contains(&node.path.value)
                                || node.score
                                    + scorecast
                                        .get_score_target_num_keys(node.path.key_set)
                                        .unwrap_or(0.0)
                                    < threshold;
                            if node.is_valid {
                                marks.mark_children(&node.path);
                                nodes_valid.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        nodes_seen.fetch_add(nodes.len(), Ordering::Relaxed);
                        let bytes = bincode::encode_to_vec(&nodes, config::standard())
                            .expect("Failed to encode nodes");
                        // Only fails if the writer panicked
                        write_sender.send((combo_set, bytes)).unwrap();
                    }
                })
            },
            |(combo_set, bytes)| chunks.write(&file, current, combo_set, &bytes),
        );

        info!(
            layout = ?chunks.layouts[current],
            nodes_seen = nodes_seen.load(Ordering::Relaxed),
            nodes_valid = nodes_valid.load(Ordering::Relaxed),
            marks = ?marks.stats(),
            elapsed = ?start.elapsed(),
            average_reader_channel_fullness = stats.calc_queue.avg_fullness(),
            average_writer_channel_fullness = stats.write_queue.avg_fullness(),
            "Finished set_full_estimate2 layout",
        );
    }
    debug_assert!(
        marks.is_empty(),
        "Marks left for chunks that were never evaluated"
    );
    marks.stats()
}

// cargo test --profile dev-opt --lib set_full_estimate2
#[cfg(test)]
mod tests {
    use gxhash::{HashMap, HashMapExt};

    use super::*;
    use crate::{
        create_set32_str, create_set32s_vec,
        general::{key_path::KeyPath, partition_id::PartitionId},
        nodes::{
            standard_node::StandardNode,
            writer::{Task, WriterWorker},
        },
        settings::NUM_KEYS,
        solver::{pending_marks::child_marks, set_full_estimate::set_full_estimate},
        util::compare_letters,
    };

    const LETTERS: &str = "abcdefg";
    // [3,2] has two parent layouts, [2,2] one, and the first two none
    const LAYOUTS: [&[usize]; 4] = [&[4, 2], &[3, 3], &[3, 2], &[2, 2]];
    const THRESHOLD: f32 = 0.3;

    #[test]
    fn child_marks_shrink_keys_with_three_or_more_letters() {
        let id = |keys: &str| PartitionId::from_keys(&create_set32s_vec!(keys));

        let marks = child_marks(&KeyPath::from_set_path(create_set32s_vec!("abc,de")));
        // Removing a letter from "de" would leave the layer
        assert_eq!(marks.len(), 3);
        for (keys, combo_set) in [("bc,de", "bcde"), ("ac,de", "acde"), ("ab,de", "abde")] {
            assert!(marks.contains(&(3, create_set32_str!(combo_set), id(keys))));
        }

        let marks = child_marks(&KeyPath::from_set_path(create_set32s_vec!("abcd,ef")));
        assert_eq!(marks.len(), 4);
        assert!(marks.contains(&(4, create_set32_str!("acdef"), id("acd,ef"))));
    }

    /// Every way to split `letters` into keys with the lengths in `layout`
    fn partitions(letters: Set32, layout: &[usize]) -> Vec<Vec<Set32>> {
        let Some(lowest) = letters.into_iter().next() else {
            return vec![vec![]];
        };
        let rest = letters.remove(lowest);
        let mut result = Vec::new();
        for (i, &len) in layout.iter().enumerate() {
            // Keys of equal length would give the same partitions again
            if i > 0 && layout[i - 1] == len {
                continue;
            }
            let others: Vec<usize> = layout[..i]
                .iter()
                .chain(&layout[i + 1..])
                .copied()
                .collect();
            for mates in rest.subsets_of_size(len as u32 - 1) {
                for mut keys in partitions(rest.difference(mates), &others) {
                    keys.push(mates.add(lowest));
                    result.push(keys);
                }
            }
        }
        result
    }

    fn score(id: PartitionId) -> f32 {
        (id.0 % 1000) as f32 / 1000.0
    }

    /// Every partition of every combo set over LETTERS for LAYOUTS, with a score derived from its ID
    fn write_layer(directory: &str) {
        for layout in LAYOUTS {
            let mut writer = WriterWorker::new(layout.to_vec());
            let num_letters = layout.iter().sum::<usize>() as u32;
            for combo_set in create_set32_str!(LETTERS).subsets_of_size(num_letters) {
                let nodes: Vec<StandardNode> = partitions(combo_set, layout)
                    .into_iter()
                    .map(|mut keys| {
                        keys.sort_by(|a, b| compare_letters(*b, *a));
                        let mut node = StandardNode::pre_eval(KeyPath::from_set_path(keys));
                        node.score = score(node.path.value);
                        node
                    })
                    .collect();
                writer.process_task_pre_encoded(Task {
                    target_combo_set: combo_set,
                    nodes_pre_encoded: bincode::encode_to_vec(&nodes, config::standard()).unwrap(),
                    directory: directory.to_string(),
                });
            }
            writer.write_node_metadata(directory);
        }
    }

    /// (is_valid_set, is_valid) of every node
    fn read_flags(directory: &str) -> HashMap<PartitionId, (bool, bool)> {
        let chunks = LayerChunks::load(directory, LAYOUTS.iter().map(|l| l.to_vec()).collect());
        let mut flags = HashMap::new();
        for layout in 0..chunks.layouts.len() {
            let file = chunks.open(layout);
            for &combo_set in chunks.metadata[layout].items.keys() {
                for node in chunks.read(&file, layout, combo_set) {
                    flags.insert(node.path.value, (node.is_valid_set, node.is_valid));
                }
            }
        }
        flags
    }

    #[test]
    fn same_flags_as_set_full_estimate() {
        let scorecast = Scorecast::new(NUM_KEYS, Set32::EMPTY);
        let layouts: Vec<Vec<usize>> = LAYOUTS.iter().map(|l| l.to_vec()).collect();
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().to_str().unwrap();
        write_layer(directory);
        set_full_estimate(&scorecast, &layouts, directory, THRESHOLD);
        let expected = read_flags(directory);

        assert!(expected.values().all(|&(is_valid_set, _)| is_valid_set));
        // Some nodes are only valid because of a parent, and some aren't valid at all
        assert!(expected
            .iter()
            .any(|(&id, &(_, is_valid))| is_valid && score(id) >= THRESHOLD));
        assert!(expected.values().any(|&(_, is_valid)| !is_valid));

        // usize::MAX never flushes, 64 B flushes on almost every task
        for cap in [usize::MAX, 64] {
            let dir = tempfile::tempdir().unwrap();
            let directory = dir.path().to_str().unwrap();
            write_layer(directory);
            let stats = set_full_estimate2(&scorecast, &layouts, directory, THRESHOLD, cap);
            assert_eq!(read_flags(directory), expected, "cap {cap}");
            assert_eq!(stats.freezes > 0, cap == 64, "cap {cap}: {stats:?}");
            assert_eq!(stats.pending_bytes, 0);
        }
    }
}
