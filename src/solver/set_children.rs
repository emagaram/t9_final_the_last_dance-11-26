use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use bincode::config;
use gxhash::HashMap;
use itertools::Itertools;
use tracing::{debug, info, span, Level};

use crate::evaluation::evaluate::config_to_opt_letter_map_fast;
use crate::evaluation::final_evaluate::final_evaluate;
use crate::evaluation::final_evaluate::path_scores::PathScores;
use crate::general::key_path::KeyPath;
use crate::general::partition_id::PartitionId;
use crate::nodes::node::{get_node_dir, get_node_path};
use crate::nodes::node_metadata::{NodeMetadata, NodeType};
use crate::nodes::predecessor::{get_full_path_from_predecessor, get_predecessor_layouts};
use crate::nodes::reader::{new_process_tasks, TasksData};
use crate::nodes::standard_node::StandardNode;
use crate::nodes::writer::{Task, WriterWorker};
use crate::scorecast::{ais_bigrams::AISBigrams, scorecast::Scorecast};
use crate::state::State;

use crate::settings::{PRINT_NODE_EVERY, PRINT_TASK_EVERY, READ_BATCH_SIZE};
use crate::solver::pipeline::run_pipeline;
use crate::solver::util::get_max_key_length;
use crate::util::{compare_letters, set32_to_string, set32s_to_string, ALPHABET_SET32};
use crate::{
    evaluation::final_evaluate::scores_hm_tree_node::StaticTrie32, general::set32::Set32,
    globals::NUM_LETTERS,
};

/// Metadata and opened data file of each predecessor of `layout`, in `get_predecessor_layouts` order.
/// None if any of them has no metadata.
fn get_predecessor_data(
    directory: &str,
    layout: &Vec<usize>,
) -> Option<(Vec<NodeMetadata>, Vec<File>)> {
    let mut metadatas = Vec::new();
    let mut files = Vec::new();
    for (_, pred_layout) in get_predecessor_layouts(layout) {
        let Some(metadata) =
            NodeMetadata::get_node_metadata(directory, &pred_layout, NodeType::Standard)
        else {
            info!(layout = ?pred_layout, "Failed to get node metadata for layout");
            return None;
        };
        let path = get_node_path(directory, &metadata.layout, metadata.node_type);
        files.push(File::open(path).expect("Failed to open predecessor file"));
        metadatas.push(metadata);
    }
    Some((metadatas, files))
}

pub fn set_children(
    base_layout: &Vec<usize>,
    target_num_keys: usize,
    hm_static_tree: &HashMap<Set32, StaticTrie32>,
    directory: &str,
    threshold: f32,
    state: &mut State,
) {
    state
        .nodes_created_layer_counter
        .store(0, Ordering::Release);
    let span_set_children = span!(Level::DEBUG, "set_children", base_layout = ?base_layout);
    let _enter_children = span_set_children.enter();

    // let max_range_end: usize = NUM_LETTERS - layout_config.iter().sum();
    let max_key_len = get_max_key_length(base_layout, target_num_keys, NUM_LETTERS);
    for key_len in 2..=max_key_len {
        let key_start = Instant::now();
        let mut full_layout = base_layout.to_vec();
        full_layout.push(key_len);

        // Metadata exists is a sign we have fully processed this layout
        if state.seen.contains(&full_layout) {
            continue;
        } else {
            let dir = get_node_dir(directory, &full_layout, NodeType::Standard);
            if Path::new(&dir).exists() {
                info!(?dir, "Removing existing directory for layout");
                std::fs::remove_dir_all(dir).expect("Failed to remove directory");
            }
        }

        let span_set_layout = span!(Level::INFO, "set_layout", full_layout = ?full_layout);
        let _enter_layout = span_set_layout.enter();

        let Some((predecessors_metadata, pred_files)) =
            get_predecessor_data(directory, &full_layout)
        else {
            break;
        };

        let full_layout_num_letters: usize = full_layout.iter().sum();
        let target_combo_sets: Vec<Set32> = ALPHABET_SET32
            .subsets_of_size(full_layout_num_letters as u32)
            .collect();
        let tasks = target_combo_sets.rchunks(READ_BATCH_SIZE);
        let tasks_sent = tasks.len();
        let tasks_received_counter = AtomicUsize::new(0);
        let mut writer = WriterWorker::new(full_layout.clone());

        let scorecast = &state.scorecast;
        let ais_bigrams = &state.ais_bigrams;
        let nodes_seen_layer_counter = &state.nodes_seen_layer_counter;
        let total_seen_layer_counter = &state.total_nodes_seen_counter;
        let nodes_created_layer_counter = &state.nodes_created_layer_counter;
        let total_nodes_created_counter = &state.total_nodes_created_counter;

        let stats = run_pipeline(
            tasks,
            |batch| {
                new_process_tasks(TasksData {
                    target_combo_sets: batch,
                    target_layout: &full_layout,
                    predecessors_metadata: &predecessors_metadata,
                    predecessors_files: &pred_files,
                })
            },
            |results, write_sender| {
                let tasks_received = tasks_received_counter.fetch_add(1, Ordering::Relaxed) + 1;
                let should_log_tasks = tasks_received % PRINT_TASK_EVERY == 0;

                let base_predecessors = results.base_nodes;
                let predecessors_map: HashMap<PartitionId, PathScores> = results.predecessors_map;
                if should_log_tasks {
                    let span_tasks = span!(
                        Level::DEBUG,
                        "set_children_tasks",
                        tasks_sent = tasks_sent,
                        tasks_received = tasks_received
                    );
                    let _enter_tasks = span_tasks.enter();
                    let base_predecessors_paths: Vec<String> = base_predecessors
                        .iter()
                        .flat_map(|(k, v)| {
                            v.iter().map(|node| {
                                format!(
                                    "({})[{}]",
                                    set32_to_string(*k),
                                    set32s_to_string(&node.path.keys)
                                )
                            })
                        })
                        .collect_vec();
                    let predecessors_strings = predecessors_map
                        .iter()
                        .map(|(k, _)| format!("[{}]", set32s_to_string(&k.to_keys())))
                        .collect_vec();
                    debug!(base_predecessors_paths = ?base_predecessors_paths);
                    debug!(predecessors_map = ?predecessors_strings);
                }
                for (target_combo_set, base_nodes) in base_predecessors {
                    let mut new_node_chunk: Vec<StandardNode> = Vec::new();
                    for base_node in base_nodes {
                        let should_log_node = get_should_log(
                            nodes_seen_layer_counter.fetch_add(1, Ordering::Relaxed),
                            PRINT_NODE_EVERY,
                        );
                        total_seen_layer_counter.fetch_add(1, Ordering::Relaxed);

                        // target combo set: ABCDEFGH, base_nodes=[ABC DE FG] (H) ABC FGH DE (ABC FGH)
                        let full_path = get_full_path_from_predecessor(
                            full_layout.len(),
                            base_node.path.keys,
                            base_node.path.key_set,
                            target_combo_set,
                        );
                        if full_path.is_none() {
                            continue;
                        }
                        let new_keypath = KeyPath::from_set_path(full_path.unwrap());
                        #[cfg(debug_assertions)]
                        {
                            for i in 1..new_keypath.keys.len() {
                                if compare_letters(new_keypath.keys[i], new_keypath.keys[i - 1])
                                    == std::cmp::Ordering::Greater
                                {
                                    panic!(
                                        "Invalid keypath generated {} for target layout {:?}",
                                        set32s_to_string(&new_keypath.keys),
                                        full_layout
                                    )
                                }
                            }
                        }
                        let node_to_evaluate = StandardNode::pre_eval(new_keypath);
                        evaluate_node(
                            node_to_evaluate,
                            &mut new_node_chunk,
                            &predecessors_map,
                            hm_static_tree,
                            scorecast,
                            ais_bigrams,
                            threshold,
                            should_log_node,
                            vec![nodes_created_layer_counter, total_nodes_created_counter],
                        );
                    }
                    if !new_node_chunk.is_empty() {
                        write_sender
                            .send(Task {
                                target_combo_set,
                                nodes_pre_encoded: bincode::encode_to_vec(
                                    &new_node_chunk,
                                    config::standard(),
                                )
                                .expect("Failed to encode nodes"),
                                directory: directory.to_string(),
                            })
                            .unwrap();
                    }
                }
            },
            |task| writer.process_task_pre_encoded(task),
        );

        if writer.node_metadata.items.is_empty() {
            info!(layout = ?full_layout, "No valid nodes found for layout");
            break;
        }
        writer.write_node_metadata(directory);

        info!(
            layout = ?full_layout,
            nodes_created_for_layer = state.nodes_created_layer_counter.load(Ordering::Relaxed),
            total_nodes_created = state
            .total_nodes_created_counter
            .load(std::sync::atomic::Ordering::Relaxed),
            average_reader_channel_length = stats.calc_queue.avg_len(),
            average_reader_channel_fullness = stats.calc_queue.avg_fullness(),
            average_writer_channel_length = stats.write_queue.avg_len(),
            average_writer_channel_fullness = stats.write_queue.avg_fullness(),
            "Added nodes for layout",
        );
        state.next_layouts.push(full_layout.clone());
        state.seen.insert(full_layout);
        let key_end = key_start.elapsed();
        state.total_elapsed_set_children += key_end;
        state.save();
    }
}

fn get_should_log(nodes_seen: usize, print_every: usize) -> bool {
    nodes_seen % print_every == 0
}

/// TODO: Refactor this function to be faster, parent and ancestor probably both do not need to be passed
fn evaluate_node(
    mut node: StandardNode,
    new_node_chunk: &mut Vec<StandardNode>,
    predecessors_map: &HashMap<PartitionId, PathScores>,
    hm_static_tree: &HashMap<Set32, StaticTrie32>,
    scorecast: &Scorecast,
    ais_bigrams: &AISBigrams,
    threshold: f32,
    should_log: bool,
    counters: Vec<&AtomicUsize>,
) {
    let letter_map_eval = config_to_opt_letter_map_fast(node.path.keys.iter());
    let eval = final_evaluate::final_evaluate_bigrams(
        &node.path.keys,
        node.path.key_set,
        &letter_map_eval,
        predecessors_map,
        hm_static_tree,
        should_log,
    );
    if eval.is_none() {
        return;
    }
    let path_scores = eval.unwrap();
    let score = path_scores.total();
    if score > threshold {
        return;
    }
    for (bigram, interaction) in path_scores.bigram_totals() {
        let other_letters = node.path.key_set.difference(bigram);
        // A lone 2-letter key has nothing to interact with
        if other_letters.is_empty() {
            continue;
        }
        ais_bigrams.update_bigram_base_scores(bigram, other_letters, interaction);
    }
    node.score = score;
    node.path_scores = path_scores;
    // Do this at the end now
    // scorecast.update_current_base_scores(node.path.key_set, score);
    new_node_chunk.push(node);
    for counter in counters {
        counter.fetch_add(1, Ordering::Relaxed);
    }
    // NODES_CREATED_LAYER_COUNTER.fetch_add(1, Ordering::Relaxed);
}
