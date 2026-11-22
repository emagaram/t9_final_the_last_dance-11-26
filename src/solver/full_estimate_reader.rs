use std::{fs::File, sync::Arc, thread};

use crossbeam_channel::{Receiver, Sender};
use gxhash::{HashSet, HashSetExt};

use crate::{
    general::set32::Set32,
    nodes::{node::Node, node_metadata::NodeMetadata, standard_node::StandardNode},
    util::ALPHABET_SET32,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoTaskType {
    Read,
    Shutdown,
}

pub struct IoTask {
    pub task_type: IoTaskType,
    pub data: Option<TasksData>,
    pub result_sender: Sender<TasksResult>,
}

pub struct TasksResult {
    pub parents: HashSet<u128>,
    pub nodes_to_eval: Vec<(Set32, Vec<StandardNode>)>,
}
pub struct TasksData {
    pub target_combo_sets: Vec<Set32>,
    pub target_metadata: Arc<NodeMetadata>,
    pub target_file: Arc<File>,
    pub parents_metadata: Arc<Vec<NodeMetadata>>,
    pub parents_files: Arc<Vec<File>>,
}

pub struct FullEstimateReader {
    pub task_sender: Sender<IoTask>,
    pub task_receiver: Receiver<IoTask>,
    worker_threads: Vec<thread::JoinHandle<()>>,
}

impl FullEstimateReader {
    pub fn get_multiple_nodes_w_file(
        combo_sets: &Vec<Set32>,
        metadata: &NodeMetadata,
        file: &File,
    ) -> Vec<(Set32, Vec<StandardNode>)> {
        let mut positions: Vec<(usize, usize)> = Vec::with_capacity(combo_sets.len());
        for combo_set in combo_sets {
            let set_loc = metadata.items.get(combo_set);
            if set_loc.is_none() {
                continue;
            }
            let set_loc = set_loc.unwrap();
            positions.push((set_loc.start, set_loc.len_bytes));
        }
        let result = StandardNode::read_multiple_nodes2_nix_full_estimate(file, positions);
        result
    }

    /// TODO Make this generic, violating DRY
    pub fn process_reads_w_file(tasks: TasksData) -> TasksResult {
        let nodes_to_eval = Self::get_multiple_nodes_w_file(
            tasks.target_combo_sets.as_ref(),
            tasks.target_metadata.as_ref(),
            tasks.target_file.as_ref(),
        );
        // println!(
        //     "Processing {} nodes to eval with {} parents",
        //     nodes_to_eval.len(),
        //     tasks.parents_metadata.len()
        // );
        let parents: HashSet<u128> = tasks
            .parents_metadata
            .iter()
            .zip(tasks.parents_files.iter())
            .map(|(node_metadata, file)| {
                let metadata_sum: usize = node_metadata.layout.iter().sum();
                let mut parents_to_eval_sets: HashSet<Set32> = HashSet::new();
                for target in tasks.target_combo_sets.iter() {
                    let missing = ALPHABET_SET32.difference(*target);
                    for combo_missing in
                        missing.subsets_of_size((metadata_sum as u32) - target.count_ones() as u32)
                    {
                        parents_to_eval_sets.insert(combo_missing.union(*target));
                    }
                }
                FullEstimateReader::get_multiple_nodes_w_file(
                    &parents_to_eval_sets.into_iter().collect(),
                    node_metadata,
                    file,
                )
            })
            .flat_map(|nodess| {
                nodess
                    .into_iter()
                    .flat_map(|(_, nodes)| nodes.into_iter().map(|node| node))
                    .filter(|node| {
                        // println!(
                        //     "Checking node with path {:?}, is_valid: {}, is_valid_set: {}",
                        //     node.path.value, node.is_valid, node.is_valid_set
                        // );
                        let cond = !node.is_valid_set || (node.is_valid && node.is_valid_set);
                        // if cond {
                        //     println!(
                        //         "Node with path {:?} is valid: {}, is_valid_set: {}",
                        //         node.path.value, node.is_valid, node.is_valid_set
                        //     );
                        // }
                        cond
                    })
                    .map(|node| node.path.value)
            })
            .collect();
        TasksResult {
            parents,
            nodes_to_eval,
        }
    }

    pub fn new(num_workers: usize) -> FullEstimateReader {
        let (task_sender, task_receiver) = crossbeam_channel::unbounded::<IoTask>();
        let mut worker_threads = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let receiver = task_receiver.clone();
            let handle = thread::spawn(move || {
                while let Ok(task) = receiver.recv() {
                    if task.task_type == IoTaskType::Shutdown {
                        break;
                    }
                    let result = FullEstimateReader::process_reads_w_file(task.data.unwrap());
                    if task.result_sender.send(result).is_err() {
                        // The receiver has been dropped, probably the main thread doesn't care about the result anymore.
                        break;
                    }
                }
            });
            worker_threads.push(handle);
        }

        FullEstimateReader {
            task_sender,
            task_receiver,
            worker_threads,
        }
    }

    pub fn shutdown(&self) {
        for _ in 0..self.worker_threads.len() {
            let (result_sender, _) = crossbeam_channel::unbounded();
            self.task_sender
                .send(IoTask {
                    task_type: IoTaskType::Shutdown,
                    data: None,
                    result_sender,
                })
                .unwrap();
        }
    }

    pub fn join(self) {
        self.shutdown();
        for handle in self.worker_threads {
            handle.join().unwrap();
        }
    }
}
