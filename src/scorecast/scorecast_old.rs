use core::f32;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use serde::{Deserialize, Serialize};

use crate::general::set32::Set32;
use crate::globals::NUM_LETTERS;
use bincode::config;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorecastOld {
    /// Given a Set32 with num of current keys equal to last completed layers, returns the smallest score for that Set32 assuming target_num_keys as the final number of keys.
    pub partial_estimates_target_num_keys: HashMap<Set32, f32>,
    /// Same as partial estimates but num of current keys is higher to allow each completed layer to not contribute to the key count.
    pub partial_estimates_extra_keys: HashMap<Set32, f32>,
    pub pathkey_scores_by_layer: Vec<HashMap<Set32, f32>>, // Base scores for each layer, used to create new scores
    pub target_num_keys: usize,
    pub full_estimates: Vec<f32>, // For most recent key layer, given a Set32 as index, returns the smallest score for that Set32 + its forecast score from "partial_estimates" for the current layer
}
impl ScorecastOld {
    pub fn new(target_num_keys: usize) -> Self {
        Self {
            partial_estimates_target_num_keys: HashMap::new(),
            partial_estimates_extra_keys: HashMap::new(),
            pathkey_scores_by_layer: vec![],
            target_num_keys,
            full_estimates: vec![f32::INFINITY; 1 << NUM_LETTERS],
        }
    }

    pub fn read_from_file_stream(path: &str) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let (
            pathkey_scores_by_layer,
            partial_estimates,
            partial_estimates_extra_keys,
            target_num_keys,
        ): (
            Vec<Vec<(u32, f32)>>,
            Vec<(u32, f32)>,
            Vec<(u32, f32)>,
            usize,
        ) = bincode::decode_from_std_read(&mut reader, config::standard())
            .expect("Failed to decode hm_tree data");

        fn convert(v: Vec<Vec<(u32, f32)>>) -> Vec<HashMap<Set32, f32>> {
            v.iter()
                .map(|v| v.iter().map(|(a, b)| (Set32(*a), *b)).collect())
                .collect()
        }

        let base_scores = convert(pathkey_scores_by_layer);
        let partial_estimates_target_num_keys: HashMap<Set32, f32> = partial_estimates
            .into_iter()
            .map(|(a, b)| (Set32(a), b))
            .collect();
        let partial_estimates_extra_keys = partial_estimates_extra_keys
            .into_iter()
            .map(|(a, b)| (Set32(a), b))
            .collect();

        Ok(Self {
            pathkey_scores_by_layer: base_scores,
            partial_estimates_target_num_keys,
            partial_estimates_extra_keys,
            full_estimates: Vec::new(),
            target_num_keys,
        })
    }
}
