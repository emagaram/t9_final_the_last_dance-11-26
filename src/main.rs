use ::tracing::{error, info};
use t9_project::{
    settings::{ITERATION_ID, NUM_KEYS, RESET_DIR, THRESHOLD},
    solver::solver::solver,
    tracing::writer::init_tracing,
};

fn main() {
    if RESET_DIR {
        if let Err(e) = std::fs::remove_dir_all(ITERATION_ID) {
            error!(directory = ITERATION_ID, ?e, "Failed to remove directory");
        }
    }
    init_tracing(std::path::Path::new(ITERATION_ID));
    info!(
        target = "solver",
        num_keys = NUM_KEYS,
        debug_assertions = cfg!(debug_assertions),
        "Starting solver with keys"
    );
    solver(THRESHOLD, NUM_KEYS);
}
