use std::collections::VecDeque;

use gxhash::HashMap;

use crate::{
    general::set32::Set32,
    nodes::{node::Node, node_metadata::NodeMetadata},
};

pub struct NodeChunkIterationState<T: Node, K> {
    pub range_start: Vec<Set32>,
    pub range_end: Vec<Set32>,
    pub current_index_data: usize,
    pub current_index_metadata: usize,
    pub data: VecDeque<T>,
    pub data_hm: Option<HashMap<u128, K>>,
    pub metadata: NodeMetadata,
}

impl<T: Node, K> NodeChunkIterationState<T, K> {
    pub fn new(metadata: NodeMetadata, data_hm: Option<HashMap<u128, K>>) -> Self {
        Self {
            range_start: Vec::new(),
            range_end: Vec::new(),
            current_index_data: 0,
            current_index_metadata: 0,
            data: VecDeque::new(),
            data_hm,
            metadata,
        }
    }
}
