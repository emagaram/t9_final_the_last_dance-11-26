use bincode::{Decode, Encode};

use crate::{
    general::set32::Set32,
    globals::NUM_LETTERS,
    scorecast::scorecast::{Scorecast, ScorecastEncodeable},
    util::set32_to_string,
};

/// These chars can be used as bigrams but not as part of the scorecast
const EXCLUDED_CHARS: [char; 8] = ['\'', 'z', 'p', 'x', 'k', 'j', 'v', 'b'];
/// Same mapping as `CHAR_TO_INDEX` ('\''=0, z=1 … a=26), computed at compile time.
const EXCLUDED_CHARS_SET32: Set32 = {
    let mut bits = 0u32;
    let mut i = 0;
    while i < EXCLUDED_CHARS.len() {
        let ch = EXCLUDED_CHARS[i];
        let idx = if ch == '\'' {
            0
        } else {
            b'a' as u32 + 26 - ch as u32
        };
        bits |= 1 << idx;
        i += 1;
    }
    Set32(bits)
};
const NUM_BIGRAMS: usize = NUM_LETTERS * (NUM_LETTERS - 1) / 2;

#[derive(Debug, Clone)]
pub struct AISBigrams {
    scorecasts: [Scorecast; NUM_BIGRAMS],
}

// Scorecast holds a DashMap, so (like Scorecast::save/read) each one goes through ScorecastEncodeable.
// The array length is fixed, so no length prefix is written.
impl Encode for AISBigrams {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        for scorecast in &self.scorecasts {
            ScorecastEncodeable::from_scorecast(scorecast).encode(encoder)?;
        }
        Ok(())
    }
}

impl<Context> Decode<Context> for AISBigrams {
    fn decode<D: bincode::de::Decoder<Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let mut scorecasts = Vec::with_capacity(NUM_BIGRAMS);
        for _ in 0..NUM_BIGRAMS {
            scorecasts.push(ScorecastEncodeable::decode(decoder)?.convert_to_scorecast());
        }
        Ok(Self {
            scorecasts: scorecasts
                .try_into()
                .unwrap_or_else(|_| unreachable!("decoded exactly NUM_BIGRAMS scorecasts")),
        })
    }
}
bincode::impl_borrow_decode!(AISBigrams);

impl AISBigrams {
    pub fn new(target_num_keys: usize) -> Self {
        Self {
            scorecasts: std::array::from_fn(|_| Scorecast::new(target_num_keys)),
        }
    }

    /// Triangular index of a 2-letter set: (lo, hi) with lo < hi -> hi*(hi-1)/2 + lo.
    fn bigram_index(bigram: Set32) -> usize {
        debug_assert_eq!(bigram.count_ones(), 2, "bigram must have exactly 2 letters");
        let lo = bigram.min_item().unwrap() as usize;
        let hi = bigram.max_item().unwrap() as usize;
        hi * (hi - 1) / 2 + lo
    }

    /// Inverse of `bigram_index`: hi is the largest h with h*(h-1)/2 <= idx, lo is the remainder.
    fn bigram_from_index(idx: usize) -> Set32 {
        debug_assert!(idx < NUM_BIGRAMS, "bigram index out of range");
        let mut hi = 1;
        while (hi + 1) * hi / 2 <= idx {
            hi += 1;
        }
        let lo = idx - hi * (hi - 1) / 2;
        Set32::EMPTY.add(lo as u32).add(hi as u32)
    }

    pub fn update_bigram_base_scores(&self, bigram: Set32, other_letters: Set32, score: f32) {
        let full_letters: Set32 = other_letters.union(bigram);
        if !full_letters.intersect(EXCLUDED_CHARS_SET32).is_empty() {
            return;
        }
        self.scorecasts[Self::bigram_index(bigram)].update_current_base_scores(full_letters, score);
    }
    pub fn get_bigram_score(&self, bigram: Set32, other_letters: Set32) -> Option<f32> {
        let full_letters: Set32 = other_letters.union(bigram).union(EXCLUDED_CHARS_SET32);
        self.scorecasts[Self::bigram_index(bigram)].get_score_target_num_keys(full_letters)
    }
    pub fn update_all(&mut self, threshold: f32) {
        for (idx, scorecast) in self.scorecasts.iter_mut().enumerate() {
            println!(
                "Updating scorecast for {}",
                set32_to_string(Self::bigram_from_index(idx))
            );
            scorecast.update(threshold);
        }
    }
}

// cargo test --release --lib ais_bigrams
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        create_set32_str, scorecast::scorecast::ScorecastEncodeable, util::ALPHABET_SET32,
    };
    use std::collections::HashMap;

    #[test]
    fn bigram_index_is_bijective() {
        let mut seen = [false; NUM_BIGRAMS];
        for bigram in ALPHABET_SET32.subsets_of_size(2) {
            let idx = AISBigrams::bigram_index(bigram);
            assert!(!seen[idx], "duplicate index {idx}");
            assert_eq!(AISBigrams::bigram_from_index(idx), bigram);
            seen[idx] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn excluded_chars_set32_matches_char_to_index() {
        let expected = EXCLUDED_CHARS.iter().fold(Set32::EMPTY, |set, ch| {
            set.add(crate::globals::CHAR_TO_INDEX[ch] as u32)
        });
        assert_eq!(EXCLUDED_CHARS_SET32, expected);
    }

    /// Snapshot of what `update_current_base_scores` has recorded for one bigram's scorecast.
    fn recorded(bigrams: &AISBigrams, bigram: Set32) -> HashMap<Set32, f32> {
        ScorecastEncodeable::from_scorecast(&bigrams.scorecasts[AISBigrams::bigram_index(bigram)])
            .current_pathkey_scores
    }

    #[test]
    fn update_skips_nodes_with_excluded_letters() {
        let bigrams = AISBigrams::new(10);
        let bigram = create_set32_str!("ae");

        // 'z' is excluded: nothing should be recorded, stripped or not
        bigrams.update_bigram_base_scores(bigram, create_set32_str!("tz"), 1.0);
        assert!(recorded(&bigrams, bigram).is_empty());

        // All valid letters: recorded under bigram ∪ other_letters
        bigrams.update_bigram_base_scores(bigram, create_set32_str!("ts"), 2.0);
        let scores = recorded(&bigrams, bigram);
        assert_eq!(scores.len(), 1);
        assert_eq!(scores.get(&create_set32_str!("aets")), Some(&2.0));

        // Only the target bigram's scorecast is touched
        assert!(recorded(&bigrams, create_set32_str!("at")).is_empty());
    }

    #[test]
    fn encode_decode_round_trip() {
        let mut bigrams = AISBigrams::new(10);
        let bigram = create_set32_str!("ae");
        bigrams.update_bigram_base_scores(bigram, create_set32_str!("ts"), 2.0);
        bigrams.scorecasts[AISBigrams::bigram_index(bigram)]
            .partial_estimates_target_num_keys
            .insert(create_set32_str!("no"), 3.0);

        let bytes = bincode::encode_to_vec(&bigrams, bincode::config::standard()).unwrap();
        let (decoded, len): (AISBigrams, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();

        assert_eq!(len, bytes.len());
        assert_eq!(recorded(&decoded, bigram), recorded(&bigrams, bigram));
        let idx = AISBigrams::bigram_index(bigram);
        assert_eq!(
            decoded.scorecasts[idx].partial_estimates_target_num_keys,
            bigrams.scorecasts[idx].partial_estimates_target_num_keys
        );
        assert!(decoded.scorecasts.iter().all(|s| s.target_num_keys == 10));
    }

    #[test]
    fn update_keeps_minimum_score() {
        let bigrams = AISBigrams::new(10);
        let bigram = create_set32_str!("ae");
        for score in [3.0, 1.5, 2.0] {
            bigrams.update_bigram_base_scores(bigram, create_set32_str!("ts"), score);
        }
        assert_eq!(
            recorded(&bigrams, bigram).get(&create_set32_str!("aets")),
            Some(&1.5)
        );
    }

    #[test]
    fn get_bigram_score_looks_up_complement_without_excluded_letters() {
        let mut bigrams = AISBigrams::new(2);
        let bigram = create_set32_str!("ae");
        let placed = create_set32_str!("aets");
        let complement = ALPHABET_SET32.difference(placed);

        let scorecast = &mut bigrams.scorecasts[AISBigrams::bigram_index(bigram)];
        // One completed layer -> 1 key remaining, keeps the debug_assert in the lookup satisfied
        scorecast.pathkey_scores_by_layer.push(HashMap::new());
        scorecast
            .partial_estimates_target_num_keys
            .insert(complement.difference(EXCLUDED_CHARS_SET32), 4.0);
        // Decoy: the complement with excluded letters still in it must not be used
        scorecast
            .partial_estimates_target_num_keys
            .insert(complement, 99.0);

        assert_eq!(
            bigrams.get_bigram_score(bigram, create_set32_str!("ts")),
            Some(4.0)
        );
    }
}
