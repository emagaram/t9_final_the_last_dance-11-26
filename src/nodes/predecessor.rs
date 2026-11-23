use std::cmp::Ordering;

use crate::{general::set32::Set32, util::compare_letters};

/// All layouts with minus 1 letters on a key. If a key has 2 letters, remove it.
pub fn get_predecessor_layouts(layout: &Vec<usize>) -> Vec<(usize, Vec<usize>)> {
    let mut result = Vec::new();
    for i in 0..layout.len() {
        if i != layout.len() - 1 && layout[i] == layout[i + 1] {
            continue;
        }
        if layout[i] == 2 {
            let add = layout
                .clone()
                .into_iter()
                .enumerate()
                .filter(|(j, _)| i != *j)
                .map(|(_, len)| len)
                .collect();
            result.push((i, add));
        } else {
            let mut add = layout.clone();
            add[i] -= 1;
            result.push((i, add));
        }
    }

    result
}
// Target: ABCDE 3 2
// Predecessors:  ABCD/ABCE 2 2, ABC/ABD 3

// 332
// ABCDEFGH => 0*1+1*27+2*27^2...
// BA 1*27+0*1

pub fn get_predecessor_combo_sets(layout_len: usize, target_combo_set: Set32) -> Vec<Set32> {
    if layout_len == 0 {
        return vec![Set32::EMPTY];
    }
    target_combo_set
        .subsets_of_size(layout_len as u32)
        .collect()
}

// ABDGX
// ABDX = false
// ABDG = true
// ABD = true

pub fn is_valid_base_node(target_combo_set: Set32, node_set: Set32) -> bool {
    let missing = target_combo_set.difference(node_set);
    is_valid_missing(target_combo_set, missing)
}

pub fn is_valid_missing(target_combo_set: Set32, missing: Set32) -> bool {
    let greatest_bit = missing.max_item().unwrap();
    let mask = (1 << (greatest_bit + 1)) - 1;
    target_combo_set.0 & mask == missing.0
}

pub fn get_full_path_from_predecessor(
    full_path_len: usize,
    predecessor_path: Vec<Set32>,
    predecessor_set: Set32,
    target_combo_set: Set32,
) -> Option<Vec<Set32>> {
    if predecessor_path.is_empty() {
        return Some(vec![target_combo_set]);
    }
    let missing = target_combo_set.difference(predecessor_set);
    if full_path_len != predecessor_path.len() {
        if compare_letters(missing, predecessor_path[predecessor_path.len() - 1])
            == Ordering::Greater
        {
            return None;
        }
        debug_assert_eq!(missing.count_ones(), 2);
        let mut result = predecessor_path;
        result.push(missing);
        return Some(result);
    }
    debug_assert_eq!(missing.count_ones(), 1);

    if missing.min_item().unwrap() > predecessor_path[full_path_len - 1].min_item().unwrap() {
        return None;
    }
    let mut result = predecessor_path;
    result[full_path_len - 1] = result[full_path_len - 1].union(missing);
    if result.len() > 1
        && compare_letters(result[full_path_len - 1], result[full_path_len - 2])
            == Ordering::Greater
    {
        return None;
    }
    Some(result)
}

/*

ab cd, abcde
abe cd
ab cde


ghx yz ghxyz

gh yz ghuvyz

*/
// pub fn get_full_paths_from_predecessor(
//     predecessor_path: Vec<Set32>,
//     predecessor_set: Set32,
//     target_combo_set: Set32,
// ) -> Vec<Vec<Set32>> {
//     let missing = target_combo_set.difference(predecessor_set);
//     if missing.count_ones() == 2 {
//         if compare_letters(missing, predecessor_path.last())
//         let mut v = predecessor_path;
//         v.push(missing);
//         return vec![v];
//     }
//     let mut result: Vec<Vec<Set32>> = Vec::with_capacity(predecessor_path.len());
//     for (i, path) in predecessor_path.iter().enumerate() {
//         let mut add = predecessor_path.clone();
//         add[i] = path.union(missing);
//         result.push(add);
//     }
//     result
// }

#[cfg(test)]
mod tests {
    use crate::{
        create_set32_str, create_set32s_vec,
        nodes::predecessor::{
            get_full_path_from_predecessor, get_predecessor_layouts, is_valid_base_node,
            is_valid_missing,
        },
        util::path_to_set32,
    };

    #[test]
    fn test_is_valid_base_node() {
        let target_combo_set = create_set32_str!("abcdefghij");
        let node_set = create_set32_str!("abcde");
        assert!(is_valid_base_node(target_combo_set, node_set));
        let node_set = create_set32_str!("abcdeg");
        assert!(!is_valid_base_node(target_combo_set, node_set));
    }
    #[test]
    fn test_is_valid_missing() {
        // 0b1111111111
        let target_combo_set = create_set32_str!("abcdefghij");
        // 0b1111100000
        let node_set = create_set32_str!("abcde");
        assert!(!is_valid_missing(target_combo_set, node_set));
        let node_set = create_set32_str!("fghij");
        assert!(is_valid_missing(target_combo_set, node_set));
        let node_set = create_set32_str!("gij");
        assert!(!is_valid_missing(target_combo_set, node_set));
        let node_set = create_set32_str!("j");
        assert!(is_valid_missing(target_combo_set, node_set));
        let node_set = create_set32_str!("i");
        assert!(!is_valid_missing(target_combo_set, node_set));
        let target_combo_set = create_set32_str!("abcfij");
        let node_set = create_set32_str!("fij");
        assert!(is_valid_missing(target_combo_set, node_set));
        let node_set = create_set32_str!("eij");
        assert!(!is_valid_missing(target_combo_set, node_set));
    }

    #[test]
    fn test_get_full_paths_from_predecessor() {
        // 3 3 3 => 4 3 3
        let predecessor_path = create_set32s_vec!("abc,def,ghi");
        let predecessor_set = path_to_set32(&predecessor_path);
        let target_combo_set = create_set32_str!("abcdefghij");
        let full_paths = get_full_path_from_predecessor(
            predecessor_path.len(),
            predecessor_path,
            predecessor_set,
            target_combo_set,
        );
        assert_eq!(full_paths.unwrap(), create_set32s_vec!("abc,def,ghij"));

        // def ab
        // abc de
        // 3 3 abcdef:

        // 3 3 3 => 3 3 3 2
        let predecessor_path = create_set32s_vec!("abc,def,ghi");
        let predecessor_set = path_to_set32(&predecessor_path);
        let target_combo_set = create_set32_str!("abcdefghijk");
        let full_paths = get_full_path_from_predecessor(
            predecessor_path.len() + 1,
            predecessor_path,
            predecessor_set,
            target_combo_set,
        );
        assert_eq!(full_paths.unwrap(), create_set32s_vec!("abc,def,ghi,jk"));

        // 3 3 2 => 3 3 3
        let predecessor_path = create_set32s_vec!("bci,def,gh");
        let predecessor_set = path_to_set32(&predecessor_path);
        let target_combo_set = create_set32_str!("abcdefghi");
        let full_paths = get_full_path_from_predecessor(
            predecessor_path.len(),
            predecessor_path,
            predecessor_set,
            target_combo_set,
        );
        assert_eq!(full_paths.unwrap(), create_set32s_vec!("bci,def,agh"));
    }

    #[test]
    fn test_get_predecessor_layouts() {
        let layout = vec![4, 3, 3, 2];
        let pred_layouts = get_predecessor_layouts(&layout);
        // [3,3,3,2] [4,3,2,2] [4,3,3]
        assert_eq!(pred_layouts.len(), 3);
        assert_eq!(pred_layouts[0].1, vec![3, 3, 3, 2]);
        assert_eq!(pred_layouts[1].1, vec![4, 3, 2, 2]);
        assert_eq!(pred_layouts[2].1, vec![4, 3, 3]);
        assert_eq!(pred_layouts[0].0, 0);
        assert_eq!(pred_layouts[1].0, 2);
        assert_eq!(pred_layouts[2].0, 3);

        let layout = vec![3];
        let pred_layouts = get_predecessor_layouts(&layout);
        assert_eq!(pred_layouts.len(), 1);
        assert_eq!(pred_layouts[0].1, vec![2]);
        assert_eq!(pred_layouts[0].0, 0);
    }
}
