use gxhash::{HashMap, HashMapExt};

use crate::{
    general::set32::Set32,
    nodes::{
        node::Node,
        node_metadata::{NodeMetadata, NodeMetadataItem, NodeType},
        standard_node::StandardNode,
    },
};

pub fn create_root(dir: &str) {
    let mut root = StandardNode::default();
    root.is_valid = true;
    root.is_valid_set = true;
    let (start, bytes_written) = StandardNode::write_nodes_to_disk(dir, &vec![root], &vec![]);
    let mut items = HashMap::new();
    items.insert(
        Set32::EMPTY,
        NodeMetadataItem {
            start: start,
            len_bytes: bytes_written,
        },
    );
    let pred_metadata = NodeMetadata {
        layout: vec![],
        items: items,
        node_type: NodeType::Standard,
    };
    NodeMetadata::write_node_metadata(dir, &pred_metadata);
}
