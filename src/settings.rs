pub const NUM_KEYS: usize = 10;
pub const EPSILON: f32 = 0.000001;
pub const THRESHOLD_FULL: f32 = 0.026348112615414632 + EPSILON;
// pub const THRESHOLD_20K: f32 = 0.014670087375483733 + EPSILON;
// pub const THRESHOLD_20K: f32 = 0.011670087375483733 + EPSILON; <--- FAILED
pub const THRESHOLD_20K: f32 = 0.014667493 + EPSILON;
pub const THRESHOLD: f32 = THRESHOLD_20K;
pub const BEST_20K: f32 = 0.014667493 + EPSILON;
pub const ITERATION_ID: &str = "iteration_1";
pub const RESET_DIR: bool = true;
pub const SAVE_TO_S3: bool = true;
pub const READ_BATCH_SIZE: usize = 10;
pub const READ_CALC_RATIO: usize = 2;
pub const WRITE_BUFFER_SIZE: usize = 20;
/// Use set_full_estimate2 (valid nodes mark their children) instead of set_full_estimate (nodes read their parents)
pub const USE_FULL_ESTIMATE2: bool = false;
/// Cap on set_full_estimate2's pending marks, at 16 B per marked node. Real RAM is about 2-3x this (hash set and
/// DashMap overhead). Past it, the workers freeze while the marks are written to disk.
pub const FULL_ESTIMATE_CACHE_BYTES: usize = 4 << 30;

pub const SCORECAST_THREAD_NUMS: [usize; 3] = [20, 20, 20];
/// Letters every bigram scorecast ignores. They can still be one of the bigram's own two letters.
pub const AIS_BIGRAM_EXCLUDED_CHARS: &str = "'zpxkjvb";
pub const PRINT_NODE_EVERY: usize = 500_000;
pub const PRINT_TASK_EVERY: usize = 100_000;

pub const HM_TRIE: &str = "hm_trie32_20000.bin";
pub const SOLVER_DICT: &str = "word_freq_20000.json";
