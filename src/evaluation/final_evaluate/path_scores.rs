use bincode::{Decode, Encode};

use crate::general::set32::Set32;

/// Penalties of one path, split by how many distinct letters each conditional involves.
///
/// `PathScores` (f32) is what nodes store and predecessors provide. `PathScores<f64>` is the
/// accumulator used while evaluating a path, rounded to f32 once at the end.
#[derive(Debug, Clone, Default, PartialEq, Encode, Decode)]
pub struct PathScores<T = f32> {
    /// All of the path's conditionals. Index i holds the ones on i+2 letters.
    pub by_letter_count: Vec<T>,
    /// For each 2-letter key, in path order: the interactions assigned to it (see `final_evaluate_bigrams`)
    /// as (number of letters, score) pairs, in no particular order. Counts without a score are left out.
    pub by_bigram: Vec<(Set32, Vec<(u8, T)>)>,
}

impl<T: Copy + Into<f64>> PathScores<T> {
    /// Score of the whole path
    pub fn total(&self) -> f32 {
        sum_scores(self.by_letter_count.iter().copied())
    }

    pub fn bigram_scores(&self, bigram: Set32) -> Option<&[(u8, T)]> {
        // At most 13 bigrams fit in a path, so a scan is cheaper than hashing
        self.by_bigram
            .iter()
            .find(|(b, _)| *b == bigram)
            .map(|(_, scores)| scores.as_slice())
    }

    /// Each 2-letter key with the sum of the interactions assigned to it, including the ones with none (0)
    pub fn bigram_totals(&self) -> impl Iterator<Item = (Set32, f32)> + '_ {
        self.by_bigram
            .iter()
            .map(|(bigram, scores)| (*bigram, sum_scores(scores.iter().map(|&(_, s)| s))))
    }
}

impl PathScores<f64> {
    pub fn zeros(num_letters: usize, bigrams: &[Set32]) -> Self {
        Self {
            by_letter_count: vec![0.0; num_letters - 1],
            by_bigram: bigrams.iter().map(|&b| (b, Vec::new())).collect(),
        }
    }

    /// Adds the scores of one minus-one sub-path.
    pub fn add_predecessor(&mut self, pred: &PathScores) {
        for (total, &score) in self.by_letter_count.iter_mut().zip(&pred.by_letter_count) {
            *total += score as f64;
        }
        for (bigram, scores) in self.by_bigram.iter_mut() {
            // The sub-paths that remove one of the bigram's letters don't have it as a key. Its
            // interactions use both letters, so there is nothing to add from those.
            for &(num_letters, score) in pred.bigram_scores(*bigram).unwrap_or_default() {
                add_to_pairs(scores, num_letters, score as f64);
            }
        }
    }

    /// Call once after every minus-one sub-path was added.
    pub fn divide_duplicates(&mut self) {
        let path_letters = self.by_letter_count.len() + 1;
        let by_letter_count = self.by_letter_count.iter_mut().enumerate();
        divide_scores(by_letter_count.map(|(i, s)| (i + 2, s)), path_letters);
        for (_, scores) in self.by_bigram.iter_mut() {
            divide_scores(scores.iter_mut().map(|(k, s)| (*k as usize, s)), path_letters);
        }
    }

    pub fn into_f32(self) -> PathScores {
        PathScores {
            by_letter_count: self.by_letter_count.iter().map(|&s| s as f32).collect(),
            by_bigram: self
                .by_bigram
                .into_iter()
                .map(|(bigram, scores)| {
                    let scores = scores.iter().map(|&(k, s)| (k, s as f32)).collect();
                    (bigram, scores)
                })
                .collect(),
        }
    }
}

fn add_to_pairs(scores: &mut Vec<(u8, f64)>, num_letters: u8, score: f64) {
    match scores.iter_mut().find(|(k, _)| *k == num_letters) {
        Some((_, total)) => *total += score,
        None => scores.push((num_letters, score)),
    }
}

fn sum_scores<T: Into<f64>>(scores: impl Iterator<Item = T>) -> f32 {
    scores.map(Into::<f64>::into).sum::<f64>() as f32
}

/// A conditional on k of the path's n letters is in n-k of its minus-one sub-paths, so it was added n-k times.
/// The ones on all n letters are in none of them and are left alone.
fn divide_scores<'a>(scores: impl Iterator<Item = (usize, &'a mut f64)>, path_letters: usize) {
    for (num_letters, score) in scores {
        if num_letters < path_letters {
            *score /= (path_letters - num_letters) as f64;
        }
    }
}

// cargo test --release --lib path_scores
#[cfg(test)]
mod tests {
    use super::*;
    use crate::create_set32_str;

    #[test]
    fn divide_leaves_full_path_score_alone() {
        // 4 letters: k=2 was added 2 times, k=3 once
        let ab = create_set32_str!("ab");
        let mut scores = PathScores {
            by_letter_count: vec![6.0, 5.0, 7.0],
            by_bigram: vec![(ab, vec![(4, 7.0), (2, 6.0)])],
        };
        scores.divide_duplicates();
        assert_eq!(scores.by_letter_count, vec![3.0, 5.0, 7.0]);
        assert_eq!(scores.by_bigram, vec![(ab, vec![(4, 7.0), (2, 3.0)])]);
    }

    #[test]
    fn add_predecessor_merges_pairs_and_skips_missing_bigrams() {
        let (ab, cd) = (create_set32_str!("ab"), create_set32_str!("cd"));
        let mut scores = PathScores::zeros(7, &[ab, cd]);
        scores.add_predecessor(&PathScores {
            by_letter_count: vec![1.0, 2.0, 3.0, 0.0, 0.0],
            by_bigram: vec![(ab, vec![(4, 3.0)])],
        });
        scores.add_predecessor(&PathScores {
            by_letter_count: vec![1.0, 0.0, 0.5, 4.0, 0.0],
            by_bigram: vec![(cd, vec![]), (ab, vec![(5, 4.0), (4, 0.5)])],
        });
        assert_eq!(scores.by_letter_count, vec![2.0, 2.0, 3.5, 4.0, 0.0, 0.0]);
        assert_eq!(scores.bigram_scores(ab), Some(&[(4, 3.5), (5, 4.0)][..]));
        assert_eq!(scores.bigram_scores(cd), Some(&[][..]));
        assert_eq!(scores.bigram_scores(create_set32_str!("ef")), None);
        let totals: Vec<(Set32, f32)> = scores.bigram_totals().collect();
        assert_eq!(totals, vec![(ab, 7.5), (cd, 0.0)]);
    }

    #[test]
    fn encode_decode_round_trip() {
        let scores: PathScores = PathScores {
            by_letter_count: vec![1.0, 2.0, 3.0],
            by_bigram: vec![(create_set32_str!("ab"), vec![(4, 4.0)])],
        };
        let bytes = bincode::encode_to_vec(&scores, bincode::config::standard()).unwrap();
        let (decoded, len): (PathScores, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(len, bytes.len());
        assert_eq!(decoded, scores);
    }
}
