use bincode::{config, Decode, Encode};
use gxhash::HashSet;
use std::fs::{rename, File};
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::scorecast::ais_bigrams::AISBigrams;
use crate::scorecast::scorecast::{Scorecast, ScorecastEncodeable};

#[derive(Debug)]
pub struct State {
    pub layouts_to_eval: Vec<Vec<usize>>,
    pub next_layouts: Vec<Vec<usize>>,
    pub layer: usize,
    pub scorecast: Scorecast,
    pub ais_bigrams: AISBigrams,
    pub nodes_created_layer_counter: AtomicUsize,
    pub total_nodes_created_counter: AtomicUsize,
    pub nodes_seen_layer_counter: AtomicUsize,
    pub total_nodes_seen_counter: AtomicUsize,
    pub total_elapsed_set_children: Duration,
    pub seen: HashSet<Vec<usize>>,
    pub directory: String,
}

impl State {
    pub fn serialize(&self) -> StateSerialized {
        StateSerialized {
            layouts_to_eval: self.layouts_to_eval.clone(),
            next_layouts: self.next_layouts.clone(),
            layer: self.layer,
            scorecast: ScorecastEncodeable::from_scorecast(&self.scorecast),
            ais_bigrams: self.ais_bigrams.clone(),
            nodes_created_layer_counter: self.nodes_created_layer_counter.load(Ordering::Relaxed),
            total_nodes_created_counter: self.total_nodes_created_counter.load(Ordering::Relaxed),
            nodes_seen_layer_counter: self.nodes_seen_layer_counter.load(Ordering::Relaxed),
            total_nodes_seen_counter: self.total_nodes_seen_counter.load(Ordering::Relaxed),
            total_elapsed_set_children_ms: self.total_elapsed_set_children.as_millis() as u64,
            seen: self.seen.clone(),
            directory: self.directory.clone(),
        }
    }
    pub fn deserialize(serialized: StateSerialized) -> Self {
        Self {
            layouts_to_eval: serialized.layouts_to_eval,
            next_layouts: serialized.next_layouts,
            layer: serialized.layer,
            scorecast: ScorecastEncodeable::convert_to_scorecast(serialized.scorecast),
            ais_bigrams: serialized.ais_bigrams,
            nodes_created_layer_counter: AtomicUsize::new(serialized.nodes_created_layer_counter),
            total_nodes_created_counter: AtomicUsize::new(serialized.total_nodes_created_counter),
            nodes_seen_layer_counter: AtomicUsize::new(serialized.nodes_seen_layer_counter),
            total_nodes_seen_counter: AtomicUsize::new(serialized.total_nodes_seen_counter),
            total_elapsed_set_children: Duration::from_millis(
                serialized.total_elapsed_set_children_ms,
            ),
            seen: serialized.seen,
            directory: serialized.directory,
        }
    }
    pub fn get_path(iteration_id: &str) -> String {
        format!("{}/progress.bin", iteration_id)
    }
    pub fn save(&self) {
        let final_path_str = Self::get_path(&self.directory);
        let final_path = Path::new(&final_path_str);
        let file = tempfile::Builder::new()
            .tempfile_in(final_path.parent().unwrap())
            .expect("Unable to create temp file");
        let mut writer = BufWriter::new(&file);

        bincode::encode_into_std_write(self.serialize(), &mut writer, config::standard())
            .expect("Unable to serialize progress");
        file.as_file().sync_all().expect("Unable to sync temp file");
        rename(file.path(), final_path_str).expect("Unable to rename temp file to progress file");
    }
    pub fn load(iteration_id: &str) -> Option<Self> {
        let file = File::open(Self::get_path(iteration_id)).ok()?;
        let mut reader = std::io::BufReader::new(file);

        let res: StateSerialized =
            bincode::decode_from_std_read(&mut reader, config::standard()).unwrap();
        Some(State::deserialize(res))
    }
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct StateSerialized {
    pub layouts_to_eval: Vec<Vec<usize>>,
    pub next_layouts: Vec<Vec<usize>>,
    pub layer: usize,
    pub scorecast: ScorecastEncodeable,
    pub ais_bigrams: AISBigrams,
    pub nodes_created_layer_counter: usize,
    pub total_nodes_created_counter: usize,
    pub nodes_seen_layer_counter: usize,
    pub total_nodes_seen_counter: usize,
    pub total_elapsed_set_children_ms: u64,
    pub seen: HashSet<Vec<usize>>,
    pub directory: String,
}
