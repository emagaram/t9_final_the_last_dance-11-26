use std::{collections::HashSet, fs::File};

use bincode::{Decode, Encode};

use crate::{
    general::{key_path::KeyPath, set32::Set32},
    nodes::{
        node::Node,
        node_metadata::{NodeMetadata, NodeType},
    },
};

// 3 3 3 3 => 3 3 3, 3 3 3

#[derive(Decode, Encode, Clone, Debug, PartialEq)]
pub struct StandardNode {
    pub path: KeyPath,                   //~40b
    pub score_by_letter_count: Vec<f32>, // ~100b
    pub score: f32,
    pub is_valid_set: bool, // We do this instead of Option<bool> to keep the size of the struct constant
    pub is_valid: bool,
}

impl Node for StandardNode {
    fn get_key_path(&self) -> &KeyPath {
        &self.path
    }
    const NODE_TYPE: NodeType = NodeType::Standard;
}

impl Default for StandardNode {
    fn default() -> Self {
        StandardNode {
            path: KeyPath::default(),
            score_by_letter_count: vec![],
            score: 0.0,
            is_valid_set: false,
            is_valid: false,
        }
    }
}

impl StandardNode {
    pub fn new(path: KeyPath, score_by_letter_count: Vec<f32>, score: f32) -> Self {
        StandardNode {
            path,
            score_by_letter_count,
            score,
            is_valid: false,
            is_valid_set: false,
        }
    }
    pub fn pre_eval(path: KeyPath) -> Self {
        StandardNode {
            path,
            score_by_letter_count: vec![],
            score: 0.0,
            is_valid: false,
            is_valid_set: false,
        }
    }

    pub fn calculate_predecessor_layouts(layout_config: &[usize]) -> Vec<Vec<usize>> {
        let mut predecessors = Vec::new();
        let mut seen = HashSet::new();

        for (i, &len) in layout_config.iter().enumerate() {
            let mut current = Vec::new();
            if len > 2 {
                current.push(len - 1);
            }
            for (j, &len2) in layout_config.iter().enumerate() {
                if j != i && len2 > 1 {
                    current.push(len2);
                }
            }
            current.sort_unstable_by(|a, b| b.cmp(a)); // Sort in descending order

            if !current.is_empty() {
                // Convert Vec to a format that can be used in a HashSet
                let key = current.clone();
                if seen.insert(key) {
                    predecessors.push(current);
                }
            }
        }

        predecessors
    }

    pub fn calculate_parent_predecessor_layouts(layout_config: &[usize]) -> Vec<Vec<usize>> {
        let mut predecessors = Vec::new();

        // 333 -> 433, 3332
        // 432->532,442,433
        // 433-> 533, 443
        for i in 0..layout_config.len() {
            if i != 0 && (layout_config[i] == layout_config[i - 1]) {
                continue;
            }
            let mut current = layout_config.to_vec();
            current[i] += 1;

            predecessors.push(current);
        }

        predecessors
    }

    pub fn get_multiple_nodes(
        combo_sets: &Vec<(usize, Set32, Set32)>,
        metadata: &NodeMetadata,
        directory: &str,
    ) -> Vec<(usize, Set32, StandardNode)> {
        let mut positions: Vec<(usize, Set32, (usize, usize))> =
            Vec::with_capacity(combo_sets.len());
        for (index_changed, target_combo_set, combo_set) in combo_sets {
            let set_loc = metadata.items.get(&combo_set);
            if set_loc.is_none() {
                continue;
            }
            let set_loc = set_loc.unwrap();
            positions.push((
                *index_changed,
                *target_combo_set,
                (set_loc.start, set_loc.len_bytes),
            ));
        }
        let result = StandardNode::read_multiple_nodes2(directory, &metadata.layout, positions);
        result
    }
    pub fn get_multiple_nodes_w_file(
        combo_sets: &Vec<(usize, Set32, Set32)>,
        metadata: &NodeMetadata,
        file: &File,
    ) -> Vec<(usize, Set32, StandardNode)> {
        let mut positions: Vec<(usize, Set32, (usize, usize))> =
            Vec::with_capacity(combo_sets.len());
        for (index_changed, target_combo_set, combo_set) in combo_sets {
            let set_loc = metadata.items.get(&combo_set);
            if set_loc.is_none() {
                continue;
            }
            let set_loc = set_loc.unwrap();
            positions.push((
                *index_changed,
                *target_combo_set,
                (set_loc.start, set_loc.len_bytes),
            ));
        }
        let result = StandardNode::read_multiple_nodes2_nix(file, positions);
        result
    }

    pub fn get_multiple_nodes_old(
        combo_sets: &[Set32],
        metadata: &NodeMetadata,
        directory: &str,
    ) -> Vec<StandardNode> {
        let mut result = Vec::new();
        let mut positions: Vec<(usize, usize)> = Vec::with_capacity(combo_sets.len());
        for set in combo_sets {
            let set_loc = metadata.items.get(set);
            if set_loc.is_none() {
                continue;
            }
            let set_loc = set_loc.unwrap();
            positions.push((set_loc.start, set_loc.len_bytes));
        }
        result.extend(StandardNode::read_multiple_nodes(
            directory,
            &metadata.layout,
            positions,
        ));
        result
    }
}

mod tests {

    #[test]
    fn test_get_parent_predecessors() {
        let layout_configs = [5, 4, 3];
        let predecessors =
            super::StandardNode::calculate_parent_predecessor_layouts(&layout_configs);
        assert_eq!(
            predecessors,
            vec![vec![6, 4, 3], vec![5, 5, 3], vec![5, 4, 4]]
        );
        let layout_configs = [4, 4, 4];
        let predecessors =
            super::StandardNode::calculate_parent_predecessor_layouts(&layout_configs);
        assert_eq!(predecessors, vec![vec![5, 4, 4]]);
        let layout_configs = [2];
        let predecessors =
            super::StandardNode::calculate_parent_predecessor_layouts(&layout_configs);
        assert_eq!(predecessors, vec![vec![3]]);
    }

    #[test]
    fn test_get_predecessors_reg() {
        let layout_configs = [5, 4, 3];
        let predecessors = super::StandardNode::calculate_predecessor_layouts(&layout_configs);
        assert_eq!(
            predecessors,
            vec![vec![4, 4, 3], vec![5, 3, 3], vec![5, 4, 2]]
        );
    }

    #[test]
    fn test_get_predecessors_no_one() {
        let layout_configs = [3, 2, 2];
        let predecessors = super::StandardNode::calculate_predecessor_layouts(&layout_configs);
        assert_eq!(predecessors, vec![vec![2, 2, 2], vec![3, 2]]);
    }

    #[test]
    fn test_get_predecessors_no_dup() {
        let layout_configs = [3, 3, 3];
        let predecessors = super::StandardNode::calculate_predecessor_layouts(&layout_configs);
        assert_eq!(predecessors, vec![vec![3, 3, 2]]);
    }

    #[test]
    fn test_get_predecessors_empty() {
        let layout_configs = [2];
        let predecessors = super::StandardNode::calculate_predecessor_layouts(&layout_configs);
        assert!(predecessors.is_empty());
    }
}
