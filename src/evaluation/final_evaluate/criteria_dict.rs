use crate::{general::set32::Set32, globals::NUM_LETTERS, types::LetterSeq};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub struct CriteriaDict {
    pub criteria: [[u32; NUM_LETTERS]; NUM_LETTERS],
    num_keys: u32,
}
impl CriteriaDict {
    pub fn new() -> Self {
        CriteriaDict {
            criteria: [[0; NUM_LETTERS]; NUM_LETTERS],
            num_keys: 0,
        }
    }
    pub fn add(&self, letter_seq: &LetterSeq) -> Self {
        let mut new_self = self.clone();
        let new_key_id = new_self.num_keys + 1;
        new_self.num_keys += 1;
        for (i, letter_i) in letter_seq.iter().enumerate() {
            for letter_j in letter_seq.iter().skip(i + 1) {
                new_self.criteria[*letter_i as usize][*letter_j as usize] = new_key_id;
                new_self.criteria[*letter_j as usize][*letter_i as usize] = new_key_id;
            }
        }
        new_self
    }
    pub fn criteria_check(&self, word1: &LetterSeq, word2: &LetterSeq) -> bool {
        let mut criteria_count = Set32::singleton(0);
        for (char_1, char2) in word1.iter().zip(word2.iter()) {
            let overlap = self.criteria[*char_1 as usize][*char2 as usize];
            criteria_count = criteria_count.add(overlap);
        }
        criteria_count.count_ones() == self.num_keys + 1
    }
    pub fn criteria_get(word1: &LetterSeq, word2: &LetterSeq) -> Vec<(u8, u8)> {
        let mut criteria: Vec<(u8, u8)> = vec![];
        for (char1, char2) in word1.iter().zip(word2.iter()) {
            criteria.push((*char1, *char2));
        }
        criteria
    }
}
#[cfg(test)]
mod tests {
    use crate::{create_set32_str, create_word};

    use super::*;

    #[test]
    fn test_new_criteria_dict() {
        let dict = CriteriaDict::new();
        assert_eq!(dict.num_keys, 0);
        for i in 0..NUM_LETTERS {
            for j in 0..NUM_LETTERS {
                assert_eq!(dict.criteria[i][j], 0);
            }
        }
    }

    #[test]
    fn test_add_letter_seq() {
        let dict = CriteriaDict::new();
        let letter_seq = create_set32_str!("abc")
            .ones_indices()
            .into_iter()
            .map(|x| x as u8)
            .collect::<Vec<u8>>();
        let dict = dict.add(&letter_seq);

        assert_eq!(dict.num_keys, 1);

        let a = create_set32_str!("a").ones_indices()[0] as usize;
        let b = create_set32_str!("b").ones_indices()[0] as usize;
        let c = create_set32_str!("c").ones_indices()[0] as usize;

        assert_eq!(dict.criteria[a][b], 1);
        assert_eq!(dict.criteria[b][a], 1);
        assert_eq!(dict.criteria[a][c], 1);
        assert_eq!(dict.criteria[c][a], 1);
        assert_eq!(dict.criteria[b][c], 1);
        assert_eq!(dict.criteria[c][b], 1);
        assert_eq!(dict.criteria[a][a], 0);
        assert_eq!(dict.criteria[b][b], 0);
        assert_eq!(dict.criteria[c][c], 0);
    }
    #[test]
    fn test_criteria_check() {
        let dict = CriteriaDict::new();
        let letter_seq = create_word!("abc");
        let dict = dict.add(&letter_seq);

        // Words with the same letters shouldnt meet criteria
        // Words with the same letters shouldnt meet criteria
        let word1 = create_word!("abc");
        let word2 = create_word!("abc");
        assert!(!dict.criteria_check(&word1, &word2));

        // Words with different letters should meet criteria
        let word3 = create_word!("aba");
        assert!(dict.criteria_check(&word1, &word3));
    }
    #[test]
    fn test_multiple_criteria() {
        let mut dict = CriteriaDict::new();
        let seq1 = create_word!("abc");
        let seq2 = create_word!("def");

        dict = dict.add(&seq1);
        dict = dict.add(&seq2);

        assert_eq!(dict.num_keys, 2);

        // Both criteria must be satisfied
        let word1 = create_word!("fat");
        let word2 = create_word!("fbt");

        assert!(!dict.criteria_check(&word2, &word1));
        let word1 = create_word!("dat");
        let word2 = create_word!("fbt");
        assert!(dict.criteria_check(&word2, &word1));
    }
    // RUSTFLAGS="-Awarnings" cargo test --release test_criteria_get -- --nocapture
    #[test]
    fn test_criteria_get() {
        let word1 = create_word!("abc");
        let word2 = create_word!("def");

        let criteria = CriteriaDict::criteria_get(&word1, &word2);

        // The length should be equal to the word length (3)
        assert_eq!(criteria.len(), 3);

        let a = create_word!("a")[0];
        let b = create_word!("b")[0];
        let c = create_word!("c")[0];
        let d = create_word!("d")[0];
        let e = create_word!("e")[0];
        let f = create_word!("f")[0];

        // Check that the pairs are correct (a paired with d, b with e, c with f)
        assert_eq!(criteria[0], (a, d));
        assert_eq!(criteria[1], (b, e));
        assert_eq!(criteria[2], (c, f));

        // Test with a different word order
        let word3 = create_word!("bac");

        let criteria2 = CriteriaDict::criteria_get(&word3, &word2);

        // Check that the pairs match the new word order
        assert_eq!(criteria2[0], (b, d));
        assert_eq!(criteria2[1], (a, e));
        assert_eq!(criteria2[2], (c, f));
    }
}
