use std::cmp::min;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use bincode::config;
use crossbeam_channel::bounded;
use gxhash::HashMap;
use itertools::Itertools;
use tracing::{debug, info, span, Level};

use crate::evaluation::evaluate::config_to_opt_letter_map_fast;
use crate::evaluation::final_evaluate::final_evaluate;
use crate::general::key_path::KeyPath;
use crate::general::partition_id::PartitionId;
use crate::nodes::node::{get_node_dir, get_node_path};
use crate::nodes::node_metadata::{NodeMetadata, NodeType};
use crate::nodes::predecessor::{get_full_path_from_predecessor, get_predecessor_layouts};
use crate::nodes::reader::{IoTask, IoTaskType, SetChildrenReader, TasksData};
use crate::nodes::standard_node::StandardNode;
use crate::nodes::writer::{Task, WriterWorker};
use crate::scorecast::{ais_bigrams::AISBigrams, scorecast::Scorecast};
use crate::state::State;

use crate::settings::{
    PRINT_NODE_EVERY, PRINT_TASK_EVERY, READ_BATCH_SIZE, READ_CALC_RATIO, WRITE_BUFFER_SIZE,
};
use crate::solver::util::get_max_key_length;
use crate::util::{
    compare_letters, get_num_calc_workers, get_num_read_workers, set32_to_string, set32s_to_string,
    ALPHABET_SET32,
};
use crate::{
    evaluation::final_evaluate::scores_hm_tree_node::StaticTrie32, general::set32::Set32,
    globals::NUM_LETTERS,
};

pub fn set_children(
    base_layout: &Vec<usize>,
    target_num_keys: usize,
    hm_static_tree: &HashMap<Set32, StaticTrie32>,
    directory: &str,
    threshold: f32,
    state: &mut State,
) {
    let cpu_count = num_cpus::get();
    let num_read_workers = get_num_read_workers(cpu_count, READ_CALC_RATIO);
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

        let pred_layouts = get_predecessor_layouts(&full_layout);
        let mut predecessors_metadata: Vec<NodeMetadata> = Vec::new();

        for (_, layout) in &pred_layouts {
            match { NodeMetadata::get_node_metadata(directory, &layout, NodeType::Standard) } {
                Some(metadata) => predecessors_metadata.push(metadata),
                None => {
                    info!(layout = ?layout, "Failed to get node metadata for layout");
                    break;
                }
            }
        }
        if predecessors_metadata.len() != pred_layouts.len() {
            break;
        }

        let full_layout_num_letters: usize = full_layout.iter().sum();
        let mut target_combo_sets: Vec<Set32> = ALPHABET_SET32
            .subsets_of_size(full_layout_num_letters as u32)
            .collect();
        let node_data_reader = SetChildrenReader::new(num_read_workers);
        let (result_sender, result_receiver) = bounded(num_read_workers);
        let (writer_sender, writer_receiver) = bounded(WRITE_BUFFER_SIZE);
        let writer_receiver_cap = writer_receiver.capacity().unwrap();

        let mut writer = WriterWorker::new(full_layout.clone(), writer_receiver.clone());
        let directory_cloned = directory.to_string();

        let writer_thread = thread::spawn(move || {
            let mut tasks_processed = 0;
            let mut avg_writer_channel_len: f64 = 0.0;
            while let Ok(task) = writer_receiver.recv() {
                tasks_processed += 1;
                let n = tasks_processed as f64;
                let len = (writer_receiver.len() + 1) as f64;
                avg_writer_channel_len = (avg_writer_channel_len * (n - 1.0) + len) / n;
                writer.process_task_pre_encoded(task);
            }
            if !writer.node_metadata.items.is_empty() {
                writer.write_node_metadata(&directory_cloned);
            }
            (writer, avg_writer_channel_len)
        });

        let predecessors_metadata = Arc::new(predecessors_metadata);

        let pred_files: Arc<Vec<File>> = Arc::new(
            predecessors_metadata
                .iter()
                .map(|metadata| {
                    let path = get_node_path(directory, &metadata.layout, metadata.node_type);
                    File::open(path).expect("Failed to open predecessor file")
                })
                .collect(),
        );
        debug_assert_eq!(pred_files.len(), predecessors_metadata.len());
        let mut tasks_sent = 0;
        let full_layout = Arc::new(full_layout);
        loop {
            let chunk_size: usize = min(READ_BATCH_SIZE, target_combo_sets.len());
            if chunk_size == 0 {
                break; // No more sets to process
            }
            let chunk = target_combo_sets
                .drain(target_combo_sets.len() - chunk_size..)
                .collect_vec();

            // Sends a bunch of disk reading tasks to some threads. The result_sender then forwards the disk retrieval data
            node_data_reader
                .task_sender
                .clone()
                .send(IoTask {
                    task_type: IoTaskType::Read,
                    data: Some(TasksData {
                        target_combo_sets: chunk,
                        directory: directory.to_string(),
                        target_layout: full_layout.clone(),
                        predecessors_metadata: predecessors_metadata.clone(),
                        predecessors_files: pred_files.clone(),
                    }),
                    result_sender: result_sender.clone(),
                })
                .unwrap();
            tasks_sent += 1;
        }
        drop(result_sender);
        let average_read_channel_len = {
            let scorecast = &state.scorecast;
            let ais_bigrams = &state.ais_bigrams;
            let nodes_seen_layer_counter = &state.nodes_seen_layer_counter;
            let total_seen_layer_counter = &state.total_nodes_seen_counter;
            let nodes_created_layer_counter = &state.nodes_created_layer_counter;
            let total_nodes_created_counter = &state.total_nodes_created_counter;

            thread::scope(|s| {
                let mut threads = Vec::new();
                let num_calc_workers = get_num_calc_workers(cpu_count, READ_CALC_RATIO);
                for _ in 0..num_calc_workers {
                    let writer_sender = writer_sender.clone();
                    let full_layout = full_layout.clone();
                    let result_receiver = result_receiver.clone();
                    let t = s.spawn(move || {
                        let mut avg_channel_len: f64 = 0.0;
                        let mut tasks_received = 0;
                        while let Ok(results) = result_receiver.recv() {
                            // let results: TasksResult = result_receiver.recv().expect("Failed to receive node data");
                            tasks_received += 1;
                            let n = tasks_received as f64;
                            let len = (result_receiver.len() + 1) as f64;
                            avg_channel_len = (avg_channel_len * (n - 1.0) + len) / n;
                            let should_log_tasks = tasks_received % PRINT_TASK_EVERY == 0;

                            let base_predecessors = results.base_nodes;
                            let predecessors_map: HashMap<PartitionId, Vec<f32>> =
                                results.predecessors_map;
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
                                            if compare_letters(
                                                new_keypath.keys[i],
                                                new_keypath.keys[i - 1],
                                            ) == std::cmp::Ordering::Greater
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
                                        vec![
                                            nodes_created_layer_counter,
                                            total_nodes_created_counter,
                                        ],
                                    );
                                }
                                if !new_node_chunk.is_empty() {
                                    writer_sender
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
                        }
                        (avg_channel_len, tasks_received)
                    });
                    threads.push(t);
                }
                // Aggregate averages from all threads
                let mut total_weighted_sum = 0.0;
                let mut total_tasks = 0;
                for t in threads {
                    let (avg_channel_len, tasks_received) =
                        t.join().expect("Failed to join read worker thread");
                    total_weighted_sum += avg_channel_len * tasks_received as f64;
                    total_tasks += tasks_received;
                }
                let avg_channel_len = if total_tasks > 0 {
                    total_weighted_sum / total_tasks as f64
                } else {
                    0.0
                };
                avg_channel_len
            })
        };
        drop(writer_sender);
        let (writer, avg_writer_channel_len) = writer_thread.join().unwrap();

        if writer.node_metadata.items.is_empty() {
            info!(layout = ?full_layout, "No valid nodes found for layout");
            break;
        }

        info!(
            layout = ?full_layout,
            nodes_created_for_layer = state.nodes_created_layer_counter.load(Ordering::Relaxed),
            total_nodes_created = state
            .total_nodes_created_counter
            .load(std::sync::atomic::Ordering::Relaxed),
            average_reader_channel_length = average_read_channel_len,
            average_reader_channel_fullness = ?average_read_channel_len / result_receiver.capacity().unwrap() as f64,
            average_writer_channel_length = avg_writer_channel_len,
            average_writer_channel_fullness = ?avg_writer_channel_len / writer_receiver_cap as f64,
            "Added nodes for layout",
        );
        state.next_layouts.push(full_layout.to_vec());
        state.seen.insert(full_layout.to_vec());
        node_data_reader.join();
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
    predecessors_map: &HashMap<PartitionId, Vec<f32>>,
    hm_static_tree: &HashMap<Set32, StaticTrie32>,
    scorecast: &Scorecast,
    ais_bigrams: &AISBigrams,
    threshold: f32,
    should_log: bool,
    counters: Vec<&AtomicUsize>,
) {
    let letter_map_eval = config_to_opt_letter_map_fast(node.path.keys.iter());
    let eval = final_evaluate::final_evaluate(
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
    let (score_by_letter_count, score) = eval.unwrap();
    if score > threshold {
        return;
    }
    for (key_idx, &key) in node.path.keys.iter().enumerate() {
        if key.count_ones() != 2 {
            continue;
        }
        let other_letters = node.path.key_set.difference(key);
        if let Some(interaction) = final_evaluate::bigram_interaction_score(
            &node.path.keys,
            key_idx,
            &score_by_letter_count,
            predecessors_map,
        ) {
            ais_bigrams.update_bigram_base_scores(key, other_letters, interaction);
        }
    }
    node.score = score;
    node.score_by_letter_count = score_by_letter_count;
    scorecast.update_current_base_scores(node.path.key_set, score);
    new_node_chunk.push(node);
    for counter in counters {
        counter.fetch_add(1, Ordering::Relaxed);
    }
    // NODES_CREATED_LAYER_COUNTER.fetch_add(1, Ordering::Relaxed);
}
