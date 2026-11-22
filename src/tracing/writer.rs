use std::path::Path;
use std::sync::Once;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use tracing_tree::HierarchicalLayer;

// Initialize the tracing system with thread-safe file logging
pub fn init_tracing(log_dir: &Path) {
    static INIT: Once = Once::new();

    static mut _SOLVER_GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    static mut _INFO_GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    static mut _SOLVER_META_GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;

    INIT.call_once(|| {
        // Create the log directory if it doesn't exist
        std::fs::create_dir_all(log_dir).expect("Failed to create log directory");

        // Set up file appenders with daily rotation
        let solver_file = tracing_appender::rolling::never(log_dir, "solver.log");
        let info_file = tracing_appender::rolling::never(log_dir, "info.log");

        // Create non-blocking writers (thread-safe by design)
        let (solver_writer, guard1) = tracing_appender::non_blocking(solver_file);
        let (info_writer, guard2) = tracing_appender::non_blocking(info_file);

        unsafe {
            _SOLVER_GUARD = Some(guard1);
            _INFO_GUARD = Some(guard2);
        }

        // Layer for solver.log
        let solver_layer = HierarchicalLayer::default()
            .with_writer(solver_writer)
            .with_indent_lines(true)
            .with_targets(true)
            .with_bracketed_fields(true)
            .with_ansi(false);

        let info_layer = HierarchicalLayer::default()
            .with_writer(info_writer)
            .with_indent_lines(true)
            .with_targets(true)
            .with_bracketed_fields(true)
            .with_ansi(false)
            .with_filter(LevelFilter::INFO);

        tracing_subscriber::registry()
            .with(solver_layer)
            .with(info_layer)
            .init();
    });
}
