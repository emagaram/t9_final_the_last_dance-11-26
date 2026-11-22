use crate::{
    general::set32::Set32,
    globals::NUM_LETTERS,
    settings::NUM_KEYS,
    util::{num_arr_to_num, path_to_set32, ALPHABET_SET32},
};
use micromap::Map;
use std::cmp::min;

#[derive(PartialEq)]
pub enum SmallestOrLargest {
    Smallest,
    Largest,
}

/// Modify to use trailing_ones and trailing_zeros
pub fn get_n_unseen_letters(
    keypath: &[Set32],
    n: usize,
    smallest_or_largest: SmallestOrLargest,
) -> Set32 {
    let letters_seen: Set32 = path_to_set32(keypath);
    let letters_not_seen = ALPHABET_SET32.difference(letters_seen);
    if smallest_or_largest == SmallestOrLargest::Smallest {
        return letters_not_seen
            .into_iter()
            .take(n)
            .fold(Set32::EMPTY, |acc, x| acc.union(Set32::singleton(x)));
    } else {
        return letters_not_seen
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(n)
            .fold(Set32::EMPTY, |acc, x| acc.union(Set32::singleton(x)));
    }
}

pub fn remove_key(keys: Vec<Set32>, key_to_rm: usize) -> Vec<Set32> {
    return keys
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != key_to_rm)
        .map(|(_, key)| *key)
        .collect();
}

pub fn get_children_to_eval_layout(layout: &[usize]) -> Option<Vec<usize>> {
    match layout.len() {
        1 => None,
        _ => Some(
            layout
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != layout.len() - 2)
                .map(|(_, &val)| val)
                .collect(),
        ),
    }
}

pub fn get_children_to_eval_range(
    target_layout: &[usize],
    parent_layout: &[usize],
    parent_keypath_start: Vec<Set32>,
    parent_keypath_end: Vec<Set32>,
) -> (Vec<Set32>, Vec<Set32>) {
    let (key_to_rm, _) = get_key_to_remove_children(parent_layout);
    let mut child_keypath_start: Vec<Set32> = remove_key(parent_keypath_start, key_to_rm);
    let mut child_keypath_end: Vec<Set32> = remove_key(parent_keypath_end, key_to_rm);
    let key_to_add_layout: usize = target_layout[target_layout.len() - 1];
    let add_start: Set32 = get_n_unseen_letters(
        &child_keypath_start,
        key_to_add_layout,
        SmallestOrLargest::Smallest,
    );
    let add_end: Set32 = get_n_unseen_letters(
        &child_keypath_end,
        key_to_add_layout,
        SmallestOrLargest::Largest,
    );
    child_keypath_start.push(add_start);
    child_keypath_end.push(add_end);
    return (child_keypath_start, child_keypath_end);
}
pub fn get_key_to_remove_children(layout_config: &[usize]) -> (usize, Vec<usize>) {
    if layout_config.len() == 0 {
        return (0, layout_config.to_vec());
    }
    for i in 1..layout_config.len() - 1 {
        if layout_config[i - 1] != layout_config[i] {
            return (
                i,
                layout_config
                    .iter()
                    .enumerate()
                    .filter_map(|(j, &val)| if j != i { Some(val) } else { None })
                    .collect(),
            );
        }
    }
    return (0, layout_config[1..].to_vec());
}

pub fn get_max_key_length(layout: &[usize], target_num_keys: usize, num_letters: usize) -> usize {
    // 3 3 3, 10, 27   (27 - 9) = 18 letters, 7 keys left. 12, 6*1

    let num_letters_used: usize = layout.iter().sum();
    let num_letters_left: usize = num_letters - num_letters_used;
    let num_keys_left: usize = target_num_keys - layout.len();
    let highest_possible = num_letters_left - num_keys_left + 1;
    if layout.len() == 0 {
        return highest_possible;
    }
    min(highest_possible, layout[layout.len() - 1])
}

// Layout 3 3 3, GHU TZ' WXY -> ABC GHU WXY
//               least 433: VWXY GHU TZ'
// But if we have GHU JKL XYZ -> ABC GHU TZ'
//                WXYZ GHU JKL < VWXY GHU TZ'
pub fn get_parent_predecessor_ranges(
    layout: &[usize],
    parent_start: &[Set32],
    parent_end: &[Set32],
) -> Map<usize, (Vec<Set32>, Vec<Set32>, Vec<usize>), NUM_KEYS> {
    let mut result = Map::new();

    for i in 0..parent_end.len() {
        if i != 0 && parent_start[i].count_ones() == parent_start[i - 1].count_ones() {
            continue;
        }
        let add_start: Set32 = get_n_unseen_letters(&parent_start, 1, SmallestOrLargest::Smallest);
        let add_end: Set32 = get_n_unseen_letters(&parent_end, 1, SmallestOrLargest::Largest);
        println!("Add start: {}", add_start.to_string());
        println!("Add end: {}", add_end.to_string());
        let mut start = parent_start.to_vec();
        let mut end = parent_end.to_vec();
        let mut layout = layout.to_vec();
        layout[i] += 1;

        // println!("Removing {} from start", start[i].max_item().unwrap());
        // println!("Removing {} from end", end[i].min_item().unwrap());
        start[i] = start[i].union(add_start);
        end[i] = end[i].union(add_end);

        let key = layout_to_key(&layout);
        result.insert(key, (start, end, layout));
    }
    result
}

// Target: 432
// Parent: ADXY GKL->BDXY GKL
// Pred 43
// ADXY GKL
// Pred 332
// ADX GKL Z'
// Take parent and create its full layout range: ADXY GKL-> ADXY GKL Z'-> BDXY GKL AC
// Remove one character from each of the sub groups with the goal of largest or smallest

pub fn get_predecessor_ranges(
    target_layout: &[usize],
    parent_start: &[Set32],
    parent_end: &[Set32],
) -> Map<usize, (Vec<Set32>, Vec<Set32>, Vec<usize>), NUM_KEYS> {
    let mut result = Map::new();
    let num_keys_to_add_to_layout: usize = target_layout[target_layout.len() - 1];

    let add_start: Set32 = get_n_unseen_letters(
        &parent_start,
        num_keys_to_add_to_layout,
        SmallestOrLargest::Smallest,
    );
    let add_end: Set32 = get_n_unseen_letters(
        &parent_end,
        num_keys_to_add_to_layout,
        SmallestOrLargest::Largest,
    );
    debug_assert!(add_start.count_ones() >= 2);
    debug_assert!(add_end.count_ones() >= 2);
    let mut full_start = parent_start.to_vec();
    let mut full_end = parent_end.to_vec();
    full_start.push(add_start);
    full_end.push(add_end);
    for i in 0..full_start.len() {
        if i != full_start.len() - 1 && full_start[i].count_ones() == full_start[i + 1].count_ones()
        {
            continue;
        }
        let mut start = full_start.clone();
        let mut end = full_end.clone();
        let mut layout = target_layout.to_vec();
        layout[i] -= 1;
        if layout[i] == 1 {
            layout.remove(i);
            start.remove(i);
            end.remove(i);
        } else {
            // println!("Removing {} from start", start[i].max_item().unwrap());
            // println!("Removing {} from end", end[i].min_item().unwrap());
            start[i] = start[i].remove(start[i].max_item().unwrap());
            end[i] = end[i].remove(end[i].min_item().unwrap());
        }
        let key = layout_to_key(&layout);
        result.insert(key, (start, end, layout));
    }
    result
}

/// (optional): we could make a much smaller num by making the base the max num. No reason since the largest number [2]*13 is still < 2^64.
/// Also make central logic for layout_to_key and key to layout, we might be duplicating in KeyPath
pub fn layout_to_key(layout: &[usize]) -> usize {
    num_arr_to_num(layout, NUM_LETTERS)
}

#[cfg(test)]
mod tests {
    use crate::{
        create_set32_str, create_set32s_vec,
        solver::util::{
            get_children_to_eval_range, get_key_to_remove_children, get_n_unseen_letters,
            get_parent_predecessor_ranges, layout_to_key, SmallestOrLargest,
        },
        util::set32s_to_string,
    };

    #[test]
    fn test_get_max_key_length() {
        let target_num_keys = 10;
        let num_letters = 27;
        // 3 3 3, 10, 27   (27 - 9) = 18 letters, 7 keys left. 12, 6*1
        let max_length = super::get_max_key_length(&vec![3, 3, 3], target_num_keys, num_letters);
        assert_eq!(max_length, 3);
        // 27 - 9 = 18 letters, 7 keys left. 18 - 7*1 = 11
        let max_length = super::get_max_key_length(&vec![], target_num_keys, num_letters);
        assert_eq!(max_length, 18);
        // 7 5 4, 10, 27   (27 - 16) = 11 letters, 7 keys left. 5, 6*1
        let max_length = super::get_max_key_length(&vec![7, 5, 4], target_num_keys, num_letters);
        assert_eq!(max_length, 4);
        // 7 6, 10, 27   (27 - 13) = 13 letters, 8 keys left. 13 - 7*1 = 6
        let max_length = super::get_max_key_length(&vec![7, 7], target_num_keys, num_letters);
        assert_eq!(max_length, 6); // Example expected value
    }

    #[test]
    fn test_get_children_to_eval_range() {
        let target_layout = vec![2];
        // Provide dummy or mock values for the missing arguments as appropriate for your test
        let parent_layout = vec![];
        let parent_keypath_start = vec![]; // Replace with appropriate Set32 values if needed
        let parent_keypath_end = vec![]; // Replace with appropriate Set32 values if needed
        let (start, end) = get_children_to_eval_range(
            &target_layout,
            &parent_layout,
            parent_keypath_start,
            parent_keypath_end,
        );
        println!(
            "Start: {:?}, End: {:?}",
            set32s_to_string(&start),
            set32s_to_string(&end)
        );
        // You may need to update the assertions below to match the expected output types
        // assert_eq!(start, vec![4, 3]);
        // assert_eq!(end, vec![4, 3]);
    }

    #[test]
    fn test_get_parent_predecessor_ranges() {
        let layout = vec![3, 3, 2];
        let start = create_set32s_vec!("bdx,gkl,wy");
        let end = create_set32s_vec!("adx,gkl,wy");
        let ranges = get_parent_predecessor_ranges(&layout, &start, &end);
        assert_eq!(ranges.len(), 2);
        let r0 = ranges.get(&layout_to_key(&vec![4, 3, 2])).unwrap();
        let _r1 = ranges.get(&layout_to_key(&vec![3, 3, 3])).unwrap();
        assert_eq!(r0.0, create_set32s_vec!("bdx',gkl,wy"));
        println!("r0: {}", set32s_to_string(&r0.1));
        assert_eq!(r0.1, create_set32s_vec!("abdx,gkl,wy"));
    }

    /*
    333
    S: cuv gkl,
    E: axy bcd,

        332:
        smallest: gkl uv z', cuv hyz
     */
    #[test]
    fn test_get_predecessor_ranges() {
        let target_layout = vec![4, 3, 2];
        let parent_start = create_set32s_vec!("adxy,gkl");
        let parent_end = create_set32s_vec!("bdxy,gkl");
        let ranges = super::get_predecessor_ranges(&target_layout, &parent_start, &parent_end);
        assert_eq!(ranges.len(), 3);
        let r0: usize = layout_to_key(&vec![3, 3, 2]);
        let r1: usize = layout_to_key(&vec![4, 2, 2]);
        let r2: usize = layout_to_key(&vec![4, 3]);

        assert_eq!(ranges.get(&r0).unwrap().2, vec![3, 3, 2]);
        assert_eq!(ranges.get(&r1).unwrap().2, vec![4, 2, 2]);
        assert_eq!(ranges.get(&r2).unwrap().2, vec![4, 3]);
        assert_eq!(ranges.get(&r0).unwrap().0, create_set32s_vec!("dxy,gkl,z'"));
        assert_eq!(ranges.get(&r0).unwrap().1, create_set32s_vec!("bdx,gkl,ac"));
        assert_eq!(ranges.get(&r1).unwrap().0, create_set32s_vec!("adxy,kl,z'"));
        assert_eq!(ranges.get(&r1).unwrap().1, create_set32s_vec!("bdxy,gk,ac"));

        let target_layout = vec![3, 3, 2];
        let parent_start = create_set32s_vec!("cuv,gkl");
        let parent_end = create_set32s_vec!("axy,bcd");
        let ranges = super::get_predecessor_ranges(&target_layout, &parent_start, &parent_end);
        for range in &ranges {
            println!(
                "{}->{}",
                &set32s_to_string(&range.1 .0),
                &set32s_to_string(&range.1 .1)
            )
        }
        assert_eq!(ranges.len(), 2);
        let r0 = ranges.get(&layout_to_key(&vec![3, 3])).unwrap();
        println!(
            "r0: {}->{}",
            set32s_to_string(&r0.0),
            set32s_to_string(&r0.1)
        );
        assert_eq!(r0.0, create_set32s_vec!("gkl,z'"));
    }

    #[test]
    fn test_get_n_unseen_letters() {
        let layout = vec![4, 3, 2];
        let seen = create_set32s_vec!("adxy,gkl");
        let unseen_smallest =
            get_n_unseen_letters(&seen, layout[layout.len() - 1], SmallestOrLargest::Smallest);
        assert_eq!(unseen_smallest.to_string(), "wz'");
        let unseen_largest =
            get_n_unseen_letters(&seen, layout[layout.len() - 1], SmallestOrLargest::Largest);
        assert_eq!(unseen_largest, create_set32_str!("bce"));
    }
    #[test]
    fn test_get_key_to_remove_children() {
        let layout_config = vec![3, 3, 2, 2];
        let (key_to_remove, children) = get_key_to_remove_children(&layout_config);
        assert_eq!(key_to_remove, 2);
        assert_eq!(children, vec![3, 3, 2]);
    }
}
