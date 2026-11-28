use core::f32;
use std::{
    cmp::min,
    fs::File,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use bincode::config;
use crossbeam_channel::bounded;
use tracing::{span, Level};

use crate::{
    general::{partition_id::PartitionId, set32::Set32},
    nodes::{
        node::get_node_path,
        node_metadata::{NodeMetadata, NodeType},
        standard_node::StandardNode,
    },
    scorecast::scorecast::Scorecast,
    settings::{READ_BATCH_SIZE, READ_CALC_RATIO, WRITE_BUFFER_SIZE},
    solver::{
        full_estimate_reader::{FullEstimateReader, IoTask, IoTaskType, TasksData, TasksResult},
        full_estimate_writer::{FullEstimateWriter, Task},
    },
    util::{get_num_calc_workers, get_num_read_workers, ALPHABET_SET32},
};

pub static FULL_ESTIMATE_NODES_SEEN_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub static SET_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn set_full_estimate(
    scorecast: &Scorecast,
    layouts_to_eval: &Vec<Vec<usize>>,
    directory: &str,
    threshold: f32,
) {
    let cpu_count = num_cpus::get();
    let start = Instant::now();
    let num_read_workers = get_num_read_workers(cpu_count, READ_CALC_RATIO);
    let span_set_full_estimate = span!(
        Level::INFO,
        "set_full_estimate",
        layouts_to_eval = ?layouts_to_eval
    );
    let _enter_full_estimate = span_set_full_estimate.enter();
    let mut layouts_by_sum = layouts_to_eval.clone();
    layouts_by_sum.sort_by_key(|layout| -(layout.iter().sum::<usize>() as isize));

    for layout in layouts_by_sum {
        let full_estimate_valid_counter: AtomicUsize = AtomicUsize::new(0);

        span!(Level::DEBUG, "Processing layout", layout = ?layout);
        let nodes_to_eval_metadata =
            NodeMetadata::get_node_metadata(directory, &layout, NodeType::Standard).unwrap();
        let parent_predecessor_layouts =
            StandardNode::calculate_parent_predecessor_layouts(&layout);
        let mut parent_metadatas = Vec::new();
        let mut parent_files = Vec::new();
        for parent_layout in parent_predecessor_layouts {
            let parent_pred_metadata =
                NodeMetadata::get_node_metadata(directory, &parent_layout, NodeType::Standard);
            if parent_pred_metadata.is_none() {
                continue;
            }
            let metadata_pred = parent_pred_metadata.unwrap();
            let pred_file: File = {
                let path = get_node_path(directory, &metadata_pred.layout, metadata_pred.node_type);
                File::open(path).unwrap()
            };
            parent_files.push(pred_file);
            parent_metadatas.push(metadata_pred);
        }
        let parents_metadata_arc = Arc::new(parent_metadatas);
        let parents_files_arc = Arc::new(parent_files);
        let node_data_reader = FullEstimateReader::new(num_read_workers);
        let (result_sender, result_receiver) = bounded(num_read_workers);
        let (writer_sender, writer_receiver) = bounded(WRITE_BUFFER_SIZE);
        let writer_receiver_cap = writer_receiver.capacity().unwrap();

        let mut writer = FullEstimateWriter::new(layout.clone(), writer_receiver.clone());

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

        let mut target_combo_sets: Vec<Set32> =
            nodes_to_eval_metadata.items.keys().cloned().collect();
        let nodes_to_eval_metadata_arc = Arc::new(nodes_to_eval_metadata);
        let target_file_arc = Arc::new(
            File::open(get_node_path(
                directory,
                &nodes_to_eval_metadata_arc.layout,
                nodes_to_eval_metadata_arc.node_type,
            ))
            .unwrap(),
        );

        loop {
            let chunk_size: usize = min(READ_BATCH_SIZE, target_combo_sets.len());
            if chunk_size == 0 {
                break; // No more sets to process
            }
            let chunk = target_combo_sets
                .drain(target_combo_sets.len() - chunk_size..)
                .collect();
            node_data_reader
                .task_sender
                .clone()
                .send(IoTask {
                    task_type: IoTaskType::Read,
                    data: Some(TasksData {
                        target_combo_sets: chunk,
                        target_metadata: nodes_to_eval_metadata_arc.clone(),
                        target_file: target_file_arc.clone(),
                        parents_metadata: parents_metadata_arc.clone(),
                        parents_files: parents_files_arc.clone(),
                    }),
                    result_sender: result_sender.clone(),
                })
                .unwrap();
        }
        drop(result_sender); // Close the sender to signal no more tasks
        let average_read_channel_len = {
            thread::scope(|s| {
                let num_calc_workers = get_num_calc_workers(cpu_count, READ_CALC_RATIO);
                let mut threads = Vec::new();
                for _ in 0..num_calc_workers {
                    let writer_sender = writer_sender.clone();
                    let result_receiver = result_receiver.clone();
                    let full_estimate_valid_counter = &full_estimate_valid_counter;
                    let nodes_to_eval_metadata_arc = nodes_to_eval_metadata_arc.clone();
                    let t = s.spawn(move || {
                        let mut avg_channel_len: f64 = 0.0;
                        let mut tasks_received = 0;
                        while let Ok(TasksResult {
                            nodes_to_eval,
                            parents,
                        }) = result_receiver.recv()
                        {
                            tasks_received += 1;
                            let n = tasks_received as f64;
                            let len = (result_receiver.len() + 1) as f64;
                            avg_channel_len = (avg_channel_len * (n - 1.0) + len) / n;
                            for (combo_set, mut nodes) in nodes_to_eval {
                                let missing_letters = ALPHABET_SET32.difference(combo_set);
                                for node in &mut nodes {
                                    FULL_ESTIMATE_NODES_SEEN_COUNTER
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    node.is_valid_set = true;
                                    debug_assert_eq!(combo_set, node.path.key_set);

                                    let parents_to_eval = calculate_parents_to_check(
                                        &node.path.keys,
                                        missing_letters,
                                    );

                                    if {
                                        let start = Instant::now();
                                        let a = parents_to_eval
                                            .iter()
                                            .any(|parent| parents.contains(parent));
                                        let elapsed = start.elapsed();
                                        SET_COUNTER.fetch_add(
                                            elapsed.as_nanos() as usize,
                                            Ordering::Relaxed,
                                        );
                                        a
                                    } {
                                        node.is_valid = true;
                                        full_estimate_valid_counter
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    } else {
                                        let score = node.score
                                            + scorecast
                                                .get_score_target_num_keys(node.path.key_set)
                                                .unwrap_or(0.0);
                                        if score < threshold {
                                            node.is_valid = true;
                                            full_estimate_valid_counter
                                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        }
                                    }
                                }
                                writer_sender
                                    .send(Task {
                                        target_combo_set: combo_set,
                                        nodes_pre_encoded: bincode::encode_to_vec(
                                            &nodes,
                                            config::standard(),
                                        )
                                        .expect("Failed to encode nodes"),
                                        directory: directory.to_string(),
                                        location: nodes_to_eval_metadata_arc
                                            .items
                                            .get(&combo_set)
                                            .unwrap()
                                            .start,
                                    })
                                    .unwrap();
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
        let elapsed = start.elapsed();
        drop(writer_sender);
        let (_, avg_writer_channel_len) = writer_thread.join().unwrap();
        span!(Level::INFO, "Finished processing set_full_estimate layout", nodes_seen= FULL_ESTIMATE_NODES_SEEN_COUNTER.load(std::sync::atomic::Ordering::Relaxed), nodes_set = full_estimate_valid_counter.load(std::sync::atomic::Ordering::Relaxed), layout = ?layout,
        total_time = ?elapsed,
        parents_set=?Duration::from_nanos(SET_COUNTER.load(Ordering::Relaxed) as u64),
        avg_writer_channel_len= avg_writer_channel_len,
        avg_read_channel_len = average_read_channel_len,
        average_reader_channel_fullness = ?average_read_channel_len / result_receiver.capacity().unwrap() as f64,
        average_writer_channel_fullness = ?avg_writer_channel_len / writer_receiver_cap as f64);
        node_data_reader.join();
    }
}

// ABC GHI

pub fn calculate_parents_to_check(node_path: &Vec<Set32>, missing: Set32) -> Vec<PartitionId> {
    let cap = (missing.count_ones() as usize) * node_path.len();
    let mut result = Vec::with_capacity(cap);
    let mut copy = node_path.clone();

    for i in 0..node_path.len() {
        for letter in missing.into_iter() {
            copy[i] = copy[i].add(letter);
            result.push(PartitionId::from_keys(&copy));
            copy[i] = copy[i].remove(letter);
        }
    }
    debug_assert_eq!(result.len(), cap);
    result
}

#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use bincode::config;
    use gxhash::HashSet;

    use super::*;
    use crate::{create_set32s_vec, nodes::node_metadata::NodeMetadata, util::path_to_set32};

    // RUSTFLAGS="-Awarnings" cargo test --release old_new_full -- --nocapture
    #[test]
    fn old_new_full_est_eq() {
        let file = File::open("test_data/valid_nodes_new.bin").unwrap();
        let mut reader = BufReader::new(file);
        let new_est: HashSet<Vec<Set32>> =
            bincode::decode_from_std_read(&mut reader, config::standard()).unwrap();

        let file = File::open("test_data/valid_nodes_old.bin").unwrap();
        let mut reader = BufReader::new(file);
        let old_est: HashSet<Vec<Set32>> =
            bincode::decode_from_std_read(&mut reader, config::standard()).unwrap();

        let difference = old_est.difference(&new_est).count();
        println!(
            "New estimate: {}, Old estimate: {}, diff: {}",
            new_est.len(),
            old_est.len(),
            difference
        );
        assert_eq!(old_est, new_est);
    }
    #[test]
    fn test_calculate_parents_to_eval() {
        let node_path = create_set32s_vec!("ab,cd");
        let missing = ALPHABET_SET32.difference(path_to_set32(&node_path));
        let parents_to_eval = calculate_parents_to_check(&node_path, missing);
        // ab + 23, cd + 23
        assert_eq!(parents_to_eval.len(), 23 * 2);
        let id = |keys: Vec<Set32>| PartitionId::from_keys(&keys);
        assert!(parents_to_eval.contains(&id(create_set32s_vec!("abe,cd"))));
        assert!(parents_to_eval.contains(&id(create_set32s_vec!("cde,ab"))));
        // A new key is in the next layer, which doesn't exist yet
        assert!(!parents_to_eval.contains(&id(create_set32s_vec!("ab,cd,ef"))));
        let node_path = create_set32s_vec!("abc,de");
        let missing = ALPHABET_SET32.difference(path_to_set32(&node_path));
        let parents_to_eval = calculate_parents_to_check(&node_path, missing);
        // abc + 22, de + 22
        assert_eq!(parents_to_eval.len(), 22 * 2);
        assert!(parents_to_eval.contains(&id(create_set32s_vec!("abcf,de"))));
        assert!(parents_to_eval.contains(&id(create_set32s_vec!("abc,def"))));
    }
}
