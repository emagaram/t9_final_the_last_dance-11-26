use criterion::{criterion_group, criterion_main, Criterion};
// Import Set32 and ALPHABET_SET32 from your crate or module
#[cfg(test)]
fn bench_process_tasks(_c: &mut Criterion) {
    use std::{fs::File, sync::Arc};

    use t9_project::nodes::node_metadata::{NodeMetadata, NodeType};

    let target_len = 5;
    let target_layout = vec![(target_len - 1) as usize];
    let directory = "node_test_data";
    let predecessors_metadata = Arc::new(vec![NodeMetadata::get_node_metadata(
        directory,
        &target_layout,
        NodeType::Standard,
    )
    .unwrap()]);
    let _pred_files: Arc<Vec<File>> = Arc::new(
        predecessors_metadata
            .iter()
            .map(|metadata| {
                use t9_project::nodes::node::get_node_path;

                let path = get_node_path(directory, &metadata.layout, metadata.node_type);
                File::open(path).unwrap()
            })
            .collect(),
    );
}

criterion_group!(benches, bench_process_tasks);
criterion_main!(benches);
