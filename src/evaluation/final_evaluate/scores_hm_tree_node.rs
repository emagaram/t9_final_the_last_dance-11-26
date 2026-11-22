use bincode::{Decode, Encode};
use gxhash::{HashMap, HashMapExt};

use crate::{
    evaluation::final_evaluate::scores_hm_tree::save_stream32, general::set32::Set32,
    util::compare_letters,
};
use std::collections::VecDeque;

// #[derive(Debug, Clone, Ord)]
pub struct DynNode {
    pub children: Vec<DynNode>,
    pub letters: Set32, // accumulated key bits up to this node
    pub score: f64,     // score for full key at this node (0.0 if not a word)
}

impl Default for DynNode {
    fn default() -> Self {
        DynNode {
            children: vec![],
            letters: Set32::EMPTY,
            score: 0.0,
        }
    }
}
impl DynNode {
    pub fn new(value: Set32) -> Self {
        DynNode {
            children: vec![],
            letters: value,
            score: 0.0,
        }
    }
    pub fn find_in_children(&self, target: Set32) -> Option<&DynNode> {
        self.find_in_children_idx(target)
            .ok()
            .and_then(|idx| self.children.get(idx))
    }
    pub fn find_in_children_idx(&self, target: Set32) -> Result<usize, usize> {
        self.children
            .binary_search_by(|node| compare_letters(node.letters, target))
    }
}
#[derive(Encode, Decode)]
pub struct NodeInnerOld64 {
    pub score: f64, // accumulated key bits up to this node
    pub first_edge: u32,
    pub edge_count: u32,
}

#[derive(Encode, Decode)]
pub struct NodeInnerNew64 {
    pub score: f64, // accumulated key bits up to this node
    pub first_edge: u32,
    pub edge_count: u16,
}

#[derive(Encode, Decode)]
pub struct NodeInnerNew32 {
    pub score: f32, // accumulated key bits up to this node
    pub first_edge: u32,
    pub edge_count: u16,
}

#[derive(Encode, Decode)]
pub struct NodeInnerOld32 {
    pub score: f32, // accumulated key bits up to this node
    pub first_edge: u32,
    pub edge_count: u32,
}

/// The finished static trie: two flat arrays (nodes + edges).
///
/// Look-up walks `nodes[idx]`, then scans the contiguous slice
/// `edges[first .. first+count)` for the matching character.

#[derive(Decode)]
pub struct StaticTrieOld64 {
    pub nodes: Vec<NodeInnerOld64>,
    pub edges: Vec<(Set32, u32)>, // label only, child index is derived
}

#[derive(Decode)]
pub struct StaticTrieOld32 {
    pub nodes: Vec<NodeInnerOld32>,
    pub edges: Vec<(Set32, u32)>, // label only, child index is derived
}

#[derive(Decode)]
pub struct StaticTrie64 {
    pub nodes: Vec<NodeInnerNew64>,
    pub edges: Vec<Set32>, // label only, child index is derived
}

#[derive(Decode)]
pub struct StaticTrie32 {
    pub nodes: Vec<NodeInnerNew32>,
    pub edges: Vec<Set32>, // label only, child index is derived
}

#[derive(Decode)]
pub struct StaticTrieSmaller {
    pub nodes: Vec<NodeInnerOld32>,
    pub edges: Vec<Set32>, // label only, child index is derived
}

impl Encode for StaticTrieSmaller {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        // First encode the nodes vector
        self.nodes.encode(encoder)?;
        // Then encode the edges vector
        self.edges.encode(encoder)?;
        Ok(())
    }
}

impl Encode for StaticTrie64 {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        // First encode the nodes vector
        self.nodes.encode(encoder)?;
        // Then encode the edges vector
        self.edges.encode(encoder)?;
        Ok(())
    }
}

impl Encode for StaticTrie32 {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        // First encode the nodes vector
        self.nodes.encode(encoder)?;
        // Then encode the edges vector
        self.edges.encode(encoder)?;
        Ok(())
    }
}

/// Converts a HashMap<Set32, StaticTrieOld64> to a HashMap<Set32, StaticTrie> by removing the node indices from edges
pub fn convert_trie(trie64s: HashMap<Set32, StaticTrie64>) {
    let mut new_tries = HashMap::new();
    println!("Converting old trie to new trie format...");
    for (key, old_trie) in trie64s {
        let StaticTrie64 { nodes, edges } = old_trie;

        let new_nodes: Vec<NodeInnerNew32> = nodes
            .into_iter()
            .map(|node| NodeInnerNew32 {
                score: node.score as f32,
                first_edge: node.first_edge,
                edge_count: node.edge_count,
            })
            .collect();

        let new_trie = StaticTrie32 {
            nodes: new_nodes,
            edges,
        };

        new_tries.insert(key, new_trie);
    }
    println!("Conversion completed, saving new trie to disk...");
    // Remove the old trie file if it exists
    if let Err(e) = std::fs::remove_file("new_trie_full64.bin") {
        // Only print an error if the file exists but couldn't be removed
        if e.kind() != std::io::ErrorKind::NotFound {
            println!("Failed to remove old trie file: {}", e);
        }
    }
    save_stream32(&new_tries, "new_trie_full32.bin");
}

pub fn build_dyn_trie(mut paths: Vec<(Vec<Set32>, f64)>) -> DynNode {
    let mut root = DynNode::default();
    for (keys, score) in &paths {
        let mut current_node = &mut root;
        for &key in keys {
            let child_idx = current_node.find_in_children_idx(key);
            if child_idx.is_ok() {
                current_node = &mut current_node.children[child_idx.unwrap()];
            } else {
                let idx = child_idx.unwrap_err();
                let new_node = DynNode::new(key);
                current_node.children.insert(idx, new_node);
                current_node = &mut current_node.children[idx];
            }
        }
        current_node.score += score; // Set the score for the final node
    }
    paths.clear();
    paths.shrink_to_fit();
    root
}

/// Convert a pointer-based dynamic trie into a static one using **BFS**
/// numbering so that siblings sit next to each other in memory.
/// *No fancy compression, no unsafe code – just two flat Vecs.*
pub fn build_static_trie(root: DynNode) -> StaticTrie64 {
    let mut nodes: Vec<NodeInnerNew64> = Vec::new();
    let mut edges: Vec<Set32> = Vec::new();

    // BFS queues: dynamic node refs + their future static index.
    let mut q_nodes: VecDeque<DynNode> = VecDeque::new();
    let mut q_idx: VecDeque<usize> = VecDeque::new();

    // Insert root placeholder.
    nodes.push(NodeInnerNew64 {
        first_edge: 0,
        edge_count: 0,
        score: 0.0, // root has no score
    });
    q_nodes.push_back(root);
    q_idx.push_back(0);

    while let Some(dyn_node) = q_nodes.pop_front() {
        let parent_idx = q_idx.pop_front().unwrap();
        let start = edges.len();

        // Stable iteration order helps deterministic builds.
        let mut kids: Vec<(Set32, DynNode)> = dyn_node
            .children
            .into_iter()
            .map(|child| (child.letters, child))
            .collect();

        kids.sort_by(|(c1, _), (c2, _)| compare_letters(*c1, *c2));

        for (bit_set, child) in kids {
            let child_idx = nodes.len() as u32;
            edges.push(bit_set);
            nodes.push(NodeInnerNew64 {
                first_edge: 0, // filled once we visit `child`
                edge_count: 0,
                score: child.score,
            });

            q_nodes.push_back(child);
            q_idx.push_back(child_idx as usize);
        }

        // Record slice describing `dyn_node`'s children.
        nodes[parent_idx].first_edge = start as u32;
        nodes[parent_idx].edge_count = (edges.len() - start) as u16;
    }
    StaticTrie64 { nodes, edges }
}

impl StaticTrie64 {
    /// Finds a child node with the given target letter, returning its index if found
    pub fn find_child(&self, node_idx: usize, target: Set32) -> Option<usize> {
        let node = &self.nodes[node_idx];
        let start = node.first_edge as usize;
        let end = start + node.edge_count as usize;

        // Binary search through the edge slice for this node
        let edges = &self.edges[start..end];
        match edges.binary_search_by(|letter| compare_letters(*letter, target)) {
            Ok(idx) => Some(start + idx + 1), // Index of edge + 1 to get node index
            Err(_) => None,
        }
    }
    /// Iterate through the children of a given node
    /// Returns a slice of letters
    pub fn get_children(&self, node_idx: usize) -> (&[Set32], usize) {
        let node = &self.nodes[node_idx];
        let start = node.first_edge as usize;
        let end = start + node.edge_count as usize;
        (&self.edges[start..end], start + 1)
    }
    /// Get the node for a given idx
    pub fn get_node(&self, node_idx: usize) -> &NodeInnerNew64 {
        &self.nodes[node_idx]
    }
}

impl StaticTrie32 {
    /// Finds a child node with the given target letter, returning its index if found
    pub fn find_child(&self, node_idx: usize, target: Set32) -> Option<usize> {
        let node = &self.nodes[node_idx];
        let start = node.first_edge as usize;
        let end = start + node.edge_count as usize;

        // Binary search through the edge slice for this node
        let edges = &self.edges[start..end];
        match edges.binary_search_by(|letter| compare_letters(*letter, target)) {
            Ok(idx) => Some(start + idx + 1), // Index of edge + 1 to get node index
            Err(_) => None,
        }
    }
    /// Iterate through the children of a given node
    /// Returns a slice of letters
    pub fn get_children(&self, node_idx: usize) -> (&[Set32], usize) {
        let node = &self.nodes[node_idx];
        let start = node.first_edge as usize;
        let end = start + node.edge_count as usize;
        (&self.edges[start..end], start + 1)
    }
    /// Get the node for a given idx
    pub fn get_node(&self, node_idx: usize) -> &NodeInnerNew32 {
        &self.nodes[node_idx]
    }
}
