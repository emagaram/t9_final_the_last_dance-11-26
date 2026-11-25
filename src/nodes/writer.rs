use std::{
    fs::File,
    time::{Duration, Instant},
};

use crate::{
    general::set32::Set32,
    nodes::{
        node::{get_node_path, Node},
        node_metadata::{NodeMetadata, NodeMetadataItem, NodeType},
        standard_node::StandardNode,
    },
    util::get_set_file_deep,
};

pub struct Task {
    pub target_combo_set: Set32,
    pub nodes_pre_encoded: Vec<u8>,
    pub directory: String,
}

pub struct WriterWorker {
    pub file: Option<File>,
    pub node_metadata: NodeMetadata,
    pub total_time_written: Duration,
}

impl WriterWorker {
    pub fn process_task_pre_encoded(&mut self, task: Task) {
        let write_start_time = Instant::now();
        if self.file.is_none() {
            let path = get_node_path(
                &task.directory,
                &self.node_metadata.layout,
                NodeType::Standard,
            );

            self.file = Some(
                get_set_file_deep(&path, std::fs::OpenOptions::new().write(true).create(true))
                    .unwrap(),
            );
        }
        let mut file = self.file.as_mut().unwrap();
        let start =
            StandardNode::write_nodes_to_disk_pre_encoded(&task.nodes_pre_encoded, &mut file);

        self.node_metadata.items.insert(
            task.target_combo_set,
            NodeMetadataItem::new(start, task.nodes_pre_encoded.len()),
        );
        let write_end_time = write_start_time.elapsed();
        self.total_time_written += write_end_time;
    }
    pub fn write_node_metadata(&self, directory: &str) {
        NodeMetadata::write_node_metadata(directory, &self.node_metadata);
    }
    pub fn new(layout: Vec<usize>) -> WriterWorker {
        WriterWorker {
            node_metadata: NodeMetadata::new(NodeType::Standard, &layout),
            total_time_written: Duration::ZERO,
            file: None,
        }
    }
}

#[cfg(test)]
mod tests {}
