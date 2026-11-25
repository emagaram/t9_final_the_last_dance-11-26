use std::{
    sync::atomic::{fence, AtomicUsize},
    time::Duration,
};

use crate::{
    create_set32_str,
    evaluation::final_evaluate::scores_hm_tree::read_stream32,
    general::set32::Set32,
    scorecast::{ais_bigrams::AISBigrams, scorecast::Scorecast},
    settings::{AIS_BIGRAM_EXCLUDED_CHARS, HM_TRIE, ITERATION_ID, READ_CALC_RATIO},
    solver::{
        create_root::create_root, set_children::set_children, set_full_estimate::set_full_estimate,
    },
    state::State,
    util::{get_num_calc_workers, get_num_read_workers},
};
use gxhash::{HashSet, HashSetExt};
use tracing::{error, info, span, Level};

pub fn solver(threshold: f32, target_num_keys: usize) {
    let mut state: State;

    if let Some(progress_inner) = State::load(ITERATION_ID) {
        info!("Loaded progress from file");
        state = progress_inner;
    } else {
        info!("No progress file found, starting fresh.");
        create_root(ITERATION_ID);
        state = State {
            layouts_to_eval: vec![vec![]],
            next_layouts: Vec::new(),
            layer: 0,
            nodes_created_layer_counter: AtomicUsize::new(0),
            total_nodes_created_counter: AtomicUsize::new(0),
            nodes_seen_layer_counter: AtomicUsize::new(0),
            total_nodes_seen_counter: AtomicUsize::new(0),
            total_elapsed_set_children: Duration::ZERO,
            scorecast: Scorecast::new(target_num_keys, Set32::EMPTY),
            ais_bigrams: AISBigrams::new(
                target_num_keys,
                create_set32_str!(AIS_BIGRAM_EXCLUDED_CHARS),
            ),
            seen: HashSet::new(),
            directory: ITERATION_ID.to_string(),
        };
    }

    // Create the static tree for the hm trie before starting the solver
    let hm_static_tree = match read_stream32(HM_TRIE) {
        Ok(tree) => tree,
        Err(e) => {
            error!(?e, "Failed to read hm_static_tree from file");
            return;
        }
    };

    let cpu_count = num_cpus::get();
    let num_read_workers = get_num_read_workers(cpu_count, READ_CALC_RATIO);
    let num_calc_workers = get_num_calc_workers(cpu_count, READ_CALC_RATIO);

    info!(
        "Starting solver with threshold: {}, target_num_keys: {}, num_read_workers: {}, num_calc_workers: {}",
        threshold, target_num_keys, num_read_workers, num_calc_workers
    );
    while state.layer < target_num_keys {
        let layer = state.layer;
        let span_evaluate_layer = span!(Level::INFO, "evaluate_layer", 
            layer = layer,
            target_num_keys = target_num_keys,
            layouts_to_eval = ?state.layouts_to_eval);
        let _enter_layer = span_evaluate_layer.enter();
        info!(
            "layer {} with {} layouts to evaluate",
            layer,
            state.layouts_to_eval.len()
        );
        for layout_config in state.layouts_to_eval.clone().iter() {
            set_children(
                &layout_config,
                target_num_keys,
                &hm_static_tree,
                ITERATION_ID,
                threshold,
                &mut state,
            );
        }
        fence(std::sync::atomic::Ordering::Acquire);
        info!(
            nodes_seen_for_layer = state.nodes_seen_layer_counter.load(std::sync::atomic::Ordering::Relaxed),
            nodes_created_for_layer = state.nodes_created_layer_counter.load(std::sync::atomic::Ordering::Relaxed),
            total_nodes_created = state.total_nodes_created_counter.load(std::sync::atomic::Ordering::Relaxed),
            total_nodes_seen = state.total_nodes_seen_counter.load(std::sync::atomic::Ordering::Relaxed),
            elapsed_set_children_time = ?state.total_elapsed_set_children, layer_completed = layer, "Layer completed"
        );
        update_scorecast(layer, threshold, &mut state.scorecast);
        set_full_estimate(
            &state.scorecast,
            &state.next_layouts,
            &state.directory,
            threshold,
        );
        state.layouts_to_eval = state.next_layouts.clone();
        state.next_layouts.clear();
        state.layer += 1;
        state
            .nodes_created_layer_counter
            .store(0, std::sync::atomic::Ordering::Relaxed);
        state
            .nodes_seen_layer_counter
            .store(0, std::sync::atomic::Ordering::Relaxed);
        state.save();
    }
    info!(
        total_nodes_created = state
            .total_nodes_created_counter
            .load(std::sync::atomic::Ordering::Relaxed),
        total_nodes_seen = state
            .total_nodes_seen_counter
            .load(std::sync::atomic::Ordering::Relaxed),
        "Solver completed",
    );
}

fn update_scorecast(layer: usize, threshold: f32, scorecast: &mut Scorecast) {
    let scorecast_path = format!("scorecasts/scorecast_{}.bin", layer);
    if std::path::Path::new(&scorecast_path).exists() {
        match Scorecast::read(&scorecast_path) {
            Ok(loaded_scorecast) => {
                info!(layer = layer, "Loaded existing scorecast for layer");
                *scorecast = loaded_scorecast;
            }
            _ => {
                panic!("Failed to load existing scorecast");
            }
        }
    } else {
        println!("No existing scorecast found, creating a new one");
        scorecast.update(threshold);
        scorecast.save(&scorecast_path);
    }
}
