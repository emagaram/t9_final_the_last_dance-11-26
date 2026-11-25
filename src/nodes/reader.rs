use std::{fs::File, os::fd::AsRawFd};

use bincode::config;
use gxhash::{HashMap, HashMapExt, HashSet, HashSetExt};
use nix::libc::{self, pread};
use tracing::error;

use crate::{
    evaluation::final_evaluate::path_scores::PathScores,
    general::{partition_id::PartitionId, set32::Set32},
    nodes::{node_metadata::NodeMetadata, standard_node::StandardNode},
    util::set32_to_string,
};

pub struct TasksResult {
    pub base_nodes: Vec<(Set32, Vec<StandardNode>)>,
    pub predecessors_map: HashMap<PartitionId, PathScores>,
    pub target_combo_sets: Vec<Set32>,
}
pub struct TasksData<'a> {
    pub target_combo_sets: &'a [Set32],
    pub target_layout: &'a [usize],
    pub predecessors_metadata: &'a [NodeMetadata],
    pub predecessors_files: &'a [File],
}

pub fn get_multiple_nodes(
    combo_sets: &HashSet<Set32>,
    metadata: &NodeMetadata,
    file: &File,
) -> HashMap<Set32, Vec<StandardNode>> {
    let mut positions: Vec<(Set32, (usize, usize))> = Vec::with_capacity(combo_sets.len());
    for &target_combo_set in combo_sets {
        let set_loc = metadata.items.get(&target_combo_set);
        if set_loc.is_none() {
            continue;
        }
        let set_loc = set_loc.unwrap();
        positions.push((target_combo_set, (set_loc.start, set_loc.len_bytes)));
    }
    let mut result = read_multiple_nodes2_nix_local(file, positions);
    for (_, nodes) in result.iter_mut() {
        nodes.retain(|n| !n.is_valid_set || (n.is_valid_set && n.is_valid));
    }
    result
}

fn read_multiple_nodes2_nix_local(
    file: &File,
    positions: Vec<(Set32, (usize, usize))>,
) -> HashMap<Set32, Vec<StandardNode>>
where
    StandardNode: Sized + bincode::Decode<()> + Send + Sync,
{
    let fd = file.as_raw_fd();

    let mut res = HashMap::with_capacity(positions.len());

    for (target_combo_set, (start, num_bytes)) in positions {
        let mut buffer = vec![0u8; num_bytes];
        let bytes_read = unsafe {
            pread(
                fd,
                buffer.as_mut_ptr() as *mut libc::c_void,
                num_bytes as libc::size_t,
                start as libc::off_t,
            )
        };
        let mut slice = &buffer[..bytes_read as usize];
        let nodes_inner: Vec<StandardNode> =
            bincode::decode_from_std_read(&mut slice, config::standard())
                .expect("Failed to decode node");

        res.insert(target_combo_set, nodes_inner);
    }

    res
}

/*
    !!The missing letter wants to be with E - 1 other things, where E is the end. All E-1 must exist

    3 0 ABC
    AB: ab

    3 1 ABCD
    ABC: ab c, ac b, bc a <= can't do ABC D. D needs 1-1

    3 2 ABCDE
    ABCD: ab cd, ac bd, bc ab, ad bc
        can't do bcd ae, acd be <= Lesson is missing letter can't be in a 2.
        e needs 2 - 1 = 1 == a,b,c,d

    3 3 ABCDEF
    ABCDE: abc de, abd ce, abe cd, acd be, ace bd, ade bc, bcd ae, bce ad, bde ac, cde ab <= works actually
        f needs 3-1 = 2 == ab, ab, ad, ae, bc,bd,be,...de

    4 3 ABCDEFG
    ABCDEF: abcd ef, abce df, abcf de, acde bf, acdf be,... bcde af... cdef ab
            acd def
    Idea use E - 1:

    2 2 uses 2 1: abcd => ab c, ac b, bc a

    3 0 uses 2 0
    3 1 uses 3 0
    3 2 uses 3 1
    3 3 uses 3 2

    2 2 2 uses 2 2 1

    !!!!IMPORTANT!!!
    If layout ends with a 2, use prev layer 2 2 uses 2 0, 3 2 uses 3 0.
    This is because we don't have 2 1 which has the ability to gives us all possible combos of 2 2
    e.g. ab c, ac b, bc a => ab cd, ac bd, bc ad (2 2)
    so just get all target choose target - 2 from prev layer.
    For 2 2 4 choose 2 => ab ac ad bc bd cd
*/

pub fn generate_predecessor_sets(
    target_combo_sets: &[Set32],
    targets_layout_sum: usize,
) -> (HashSet<Set32>, HashSet<Set32>) {
    let minus_one_sets: HashSet<Set32> = match targets_layout_sum {
        2 => {
            let mut empty = HashSet::new();
            empty.insert(Set32::EMPTY);
            empty
        }
        _ => target_combo_sets
            .iter()
            .flat_map(|set| set.subsets_of_size((targets_layout_sum - 1) as u32))
            .collect(),
    };
    let minus_two_sets: HashSet<Set32> = match targets_layout_sum {
        2 => {
            let mut empty = HashSet::new();
            empty.insert(Set32::EMPTY);
            empty
        }
        _ => target_combo_sets
            .iter()
            .flat_map(|set| set.subsets_of_size((targets_layout_sum - 2) as u32))
            .collect(),
    };
    (minus_one_sets, minus_two_sets)
}

fn _get_base_nodes(
    tasks: &TasksData,
    predecessor_layout: &Vec<usize>,
    nodes_map: &HashMap<Set32, Vec<StandardNode>>,
) -> Vec<(Set32, Vec<StandardNode>)> {
    let curr_layout_sum: usize = predecessor_layout.iter().sum();
    let mut result = Vec::new();
    for &target_combo_set in tasks.target_combo_sets.iter() {
        if *tasks.target_layout.last().unwrap() == 2 {
            let bases: Vec<Set32> = match curr_layout_sum {
                0 => vec![Set32::EMPTY],
                _ => target_combo_set
                    .subsets_of_size(curr_layout_sum as u32)
                    .collect(),
            };
            let mut base_nodes = Vec::new();
            for base in bases {
                let n = nodes_map.get(&base).unwrap_or(&Vec::default()).clone();
                base_nodes.extend(n);
            }
            result.push((target_combo_set, base_nodes));
        } else {
            // abcdef
            // abcd => ignores abf cd
            let base = target_combo_set.remove(target_combo_set.min_item().unwrap());
            println!(
                "Base: {}, target_combo_set: {}",
                set32_to_string(base),
                set32_to_string(target_combo_set)
            );
            let base_nodes = nodes_map.get(&base);
            if let Some(base_nodes) = base_nodes {
                result.push((target_combo_set, base_nodes.clone()));
            }
        }
    }
    result
}
pub fn new_process_tasks(tasks: TasksData) -> TasksResult {
    if tasks.target_combo_sets.is_empty() {
        panic!("Should this even be called?");
    }
    let targets_layout_sum = tasks.target_combo_sets[0].count_ones() as usize;
    #[cfg(debug_assertions)]
    {
        for set in tasks.target_combo_sets.iter() {
            if set.count_ones() as usize != targets_layout_sum {
                error!(
                    target_combo_set = ?set,
                    targets_layout_sum = targets_layout_sum,
                    "Target combo set layout sum mismatch"
                );
                panic!("Target combo set layout sum mismatch");
            }
        }
    }
    let mut result: TasksResult = TasksResult {
        base_nodes: Vec::new(),
        predecessors_map: HashMap::new(),
        target_combo_sets: Vec::new(),
    };
    let (minus_one_sets, minus_two_sets) =
        generate_predecessor_sets(tasks.target_combo_sets, targets_layout_sum);

    // let a =
    for (i, (predecessor_node_metadata, file)) in tasks
        .predecessors_metadata
        .iter()
        .zip(tasks.predecessors_files.iter())
        .enumerate()
    {
        let subsets: &HashSet<Set32>;
        let curr_layout_sum: usize = predecessor_node_metadata.layout.iter().sum();
        // println!("Layyouts sum: {}", curr_layout_sum);
        let mut adding_two_letter_key = false;
        if curr_layout_sum == targets_layout_sum - 1 {
            subsets = &minus_one_sets;
        } else if curr_layout_sum == targets_layout_sum - 2 {
            subsets = &minus_two_sets;
            adding_two_letter_key = true;
        } else {
            error!(
                pred_layout = ?predecessor_node_metadata.layout,
                targets_len = targets_layout_sum,
                "Predecessor len unexpected"
            );
            panic!("Predecessor len unexpected");
        }
        let nodes_map = get_multiple_nodes(subsets, predecessor_node_metadata, &file);

        // 4 3 2 => 4 3 2 as the last
        if i == tasks.predecessors_metadata.len() - 1 {
            #[cfg(debug_assertions)]
            {
                if adding_two_letter_key {
                    assert_eq!(
                        tasks.target_layout[..tasks.target_layout.len() - 1],
                        predecessor_node_metadata.layout
                    );
                } else {
                    let mut target_pred_layout = tasks.target_layout.to_vec();
                    *target_pred_layout.last_mut().unwrap() -= 1;

                    assert_eq!(target_pred_layout, predecessor_node_metadata.layout);
                }
            }
            // TODO make get_base_nodes fn
            for &target_combo_set in tasks.target_combo_sets.iter() {
                if *tasks.target_layout.last().unwrap() == 2 {
                    let base_sets: Vec<Set32> = match curr_layout_sum {
                        0 => vec![Set32::EMPTY],
                        _ => target_combo_set
                            .subsets_of_size(curr_layout_sum as u32)
                            .collect(),
                    };
                    let mut base_nodes_for_one_target = Vec::new();
                    for base_set in base_sets {
                        let add = nodes_map.get(&base_set);
                        if add.is_some() {
                            base_nodes_for_one_target.extend(add.unwrap().clone());
                        }
                    }
                    result
                        .base_nodes
                        .push((target_combo_set, base_nodes_for_one_target));
                } else {
                    let mut total_size = 0;
                    let mut add_vecs = Vec::with_capacity(targets_layout_sum);
                    for base_set in
                        target_combo_set.subsets_of_size(target_combo_set.count_ones() - 1)
                    {
                        // TODO if add is none we should throw this whole target combo set away
                        // (I think?)
                        let add = nodes_map.get(&base_set);
                        if add.is_some() {
                            total_size += add.as_ref().unwrap().len();
                            add_vecs.push(add.unwrap());
                        }
                    }
                    // debug_assert_eq!(targets_layout_sum, add_vecs.len());
                    let mut base_nodes_for_one_target = Vec::with_capacity(total_size);
                    for add_vec in add_vecs {
                        base_nodes_for_one_target.extend(add_vec.clone());
                    }
                    result
                        .base_nodes
                        .push((target_combo_set, base_nodes_for_one_target));
                }
            }
        }
        for (_, nodes) in nodes_map {
            result
                .predecessors_map
                .extend(nodes.into_iter().map(|n| (n.path.value, n.path_scores)));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general::key_path::KeyPath;
    use crate::general::set32::Set32;

    use crate::{create_set32_str, create_set32s_vec};

    #[test]
    fn test_get_base_nodes_layer0() {
        let tasks = TasksData {
            target_combo_sets: &[create_set32_str!("abc")],
            target_layout: &[3],
            predecessors_metadata: &[],
            predecessors_files: &[],
        };
        let mut nodes_map: HashMap<Set32, Vec<StandardNode>> = HashMap::new();
        let ab_vec = vec![StandardNode::pre_eval(KeyPath::from_set_path(
            create_set32s_vec!("ab"),
        ))];
        nodes_map.insert(create_set32_str!("ab"), ab_vec.clone());
        nodes_map.insert(
            create_set32_str!("ac"),
            vec![StandardNode::pre_eval(KeyPath::from_set_path(
                create_set32s_vec!("ac"),
            ))],
        );
        nodes_map.insert(
            create_set32_str!("bc"),
            vec![StandardNode::pre_eval(KeyPath::from_set_path(
                create_set32s_vec!("bc"),
            ))],
        );
        let bn = _get_base_nodes(&tasks, &vec![2], &nodes_map);
        assert_eq!(bn.len(), 1);
        assert_eq!(bn[0].0, create_set32_str!("abc"));
        assert_eq!(bn[0].1, ab_vec);
    }

    #[test]
    fn test_get_base_nodes_layer1() {
        // Test 1
        let tasks = TasksData {
            target_combo_sets: &[create_set32_str!("abcdef")],
            target_layout: &[4, 3],
            predecessors_metadata: &[],
            predecessors_files: &[],
        };
        fn key_paths_to_node(paths: Vec<Vec<Set32>>) -> Vec<StandardNode> {
            paths
                .into_iter()
                .map(|path| StandardNode::pre_eval(KeyPath::from_set_path(path)))
                .collect()
        }
        let mut nodes_map: HashMap<Set32, Vec<StandardNode>> = HashMap::new();
        nodes_map.insert(
            create_set32_str!("abcde"),
            key_paths_to_node(vec![
                create_set32s_vec!("abc,de"),
                create_set32s_vec!("abd,ce"),
            ]),
        );
        let bn = _get_base_nodes(&tasks, &vec![3, 3], &nodes_map);
        assert_eq!(bn.len(), 1);
        assert_eq!(bn[0].1.len(), 2);
        // Test 2, reuse nodes_map
        let tasks = TasksData {
            target_combo_sets: &[create_set32_str!("abcdfg")],
            target_layout: &[4, 3],
            predecessors_metadata: &[],
            predecessors_files: &[],
        };
        let bn = _get_base_nodes(&tasks, &vec![3, 3], &nodes_map);
        assert_eq!(bn.len(), 0);
        // Test 3
        let tasks = TasksData {
            target_combo_sets: &[create_set32_str!("abcdefg")],
            target_layout: &[4, 3],
            predecessors_metadata: &[],
            predecessors_files: &[],
        };
        let _bn = _get_base_nodes(&tasks, &vec![4, 2], &nodes_map);
        let mut nodes_map: HashMap<Set32, Vec<StandardNode>> = HashMap::new();
        nodes_map.insert(
            create_set32_str!("abcdef"),
            key_paths_to_node(vec![
                create_set32s_vec!("abc,de"),
                create_set32s_vec!("abd,ce"),
            ]),
        );
    }

    #[test]
    fn test_generate_predecessor_sets() {
        let targets = vec![create_set32_str!("abc")];
        let (minus_one, minus_two) = generate_predecessor_sets(&targets, 3);

        let expected_minus_one: HashSet<Set32> = [
            create_set32_str!("ab"),
            create_set32_str!("ac"),
            create_set32_str!("bc"),
        ]
        .iter()
        .cloned()
        .collect();

        let expected_minus_two: HashSet<Set32> = [
            create_set32_str!("a"),
            create_set32_str!("b"),
            create_set32_str!("c"),
        ]
        .iter()
        .cloned()
        .collect();

        assert_eq!(minus_one, expected_minus_one);
        assert_eq!(minus_two, expected_minus_two);
    }

    #[test]
    fn test_generate_predecessor_sets_for_len_2() {
        let targets = vec![create_set32_str!("ab")];
        let (minus_one, minus_two) = generate_predecessor_sets(&targets, 2);
        let expected_empty: HashSet<Set32> = [Set32::EMPTY].iter().cloned().collect();
        assert_eq!(minus_one, expected_empty);
        assert_eq!(minus_two, expected_empty);
    }
}
