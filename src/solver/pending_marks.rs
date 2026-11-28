use std::{
    fs::{File, OpenOptions},
    os::unix::fs::FileExt,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use bincode::config;
use dashmap::DashMap;
use gxhash::{GxBuildHasher, HashMap, HashMapExt, HashSet};
use parking_lot::RwLock;
use rayon::prelude::*;

use crate::{
    general::{key_path::KeyPath, partition_id::PartitionId, set32::Set32},
    nodes::{
        node::get_node_path,
        node_metadata::{NodeMetadata, NodeType},
        predecessor::get_predecessor_layouts,
        standard_node::StandardNode,
    },
};

/// What a mark is counted as against the cap
const MARK_BYTES: usize = std::mem::size_of::<PartitionId>();

/// The layouts of one layer, for reading chunks and rewriting them in place
pub struct LayerChunks {
    pub layouts: Vec<Vec<usize>>,
    pub metadata: Vec<NodeMetadata>,
    directory: String,
}

impl LayerChunks {
    pub fn load(directory: &str, layouts: Vec<Vec<usize>>) -> Self {
        let metadata = layouts
            .iter()
            .map(|layout| {
                NodeMetadata::get_node_metadata(directory, layout, NodeType::Standard)
                    .expect("Missing node metadata for layout")
            })
            .collect();
        Self {
            layouts,
            metadata,
            directory: directory.to_string(),
        }
    }

    /// Opens a layout's node file for positional reads and writes, which are safe from many threads at once
    pub fn open(&self, layout: usize) -> File {
        let path = get_node_path(&self.directory, &self.layouts[layout], NodeType::Standard);
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("Failed to open node file")
    }

    pub fn read(&self, file: &File, layout: usize, combo_set: Set32) -> Vec<StandardNode> {
        let item = &self.metadata[layout].items[&combo_set];
        let mut bytes = vec![0; item.len_bytes];
        file.read_exact_at(&mut bytes, item.start as u64)
            .expect("Failed to read chunk");
        bincode::decode_from_slice(&bytes, config::standard())
            .expect("Failed to decode chunk")
            .0
    }

    /// Overwrites a chunk in place, so `bytes` must be exactly as long as the chunk on disk
    pub fn write(&self, file: &File, layout: usize, combo_set: Set32, bytes: &[u8]) {
        let item = &self.metadata[layout].items[&combo_set];
        debug_assert_eq!(bytes.len(), item.len_bytes, "Chunk changed size");
        file.write_all_at(bytes, item.start as u64)
            .expect("Failed to write chunk");
    }

    /// Sets `is_valid` on the nodes in `ids`, on disk
    fn apply(&self, file: &File, layout: usize, combo_set: Set32, ids: &HashSet<PartitionId>) {
        let mut nodes = self.read(file, layout, combo_set);
        let mut changed = false;
        for node in nodes
            .iter_mut()
            .filter(|node| ids.contains(&node.path.value))
        {
            changed |= !node.is_valid;
            node.is_valid = true;
        }
        debug_assert_eq!(
            nodes
                .iter()
                .filter(|node| ids.contains(&node.path.value))
                .count(),
            ids.len(),
            "Marked node missing from its chunk"
        );
        if changed {
            let bytes =
                bincode::encode_to_vec(&nodes, config::standard()).expect("Failed to encode chunk");
            self.write(file, layout, combo_set, &bytes);
        }
    }
}

/// Every same-layer child of `path`, i.e. `path` with one letter removed from a key with at least 3 letters, as
/// (length of that key, child combo set, child ID). Removing a letter from a 2-key leaves the layer, so those are
/// skipped.
pub fn child_marks(path: &KeyPath) -> Vec<(u32, Set32, PartitionId)> {
    let mut keys = path.keys.clone();
    let mut result = Vec::new();
    for i in 0..keys.len() {
        let key = keys[i];
        if key.count_ones() < 3 {
            continue;
        }
        for letter in key {
            keys[i] = key.remove(letter);
            result.push((
                key.count_ones(),
                path.key_set.remove(letter),
                PartitionId::from_keys(&keys),
            ));
        }
        keys[i] = key;
    }
    result
}

#[derive(Debug)]
pub struct MarksStats {
    pub pending_bytes: usize,
    pub peak_bytes: usize,
    pub freezes: usize,
    pub lists_flushed: usize,
    pub flush_time: Duration,
}

/// Nodes that a parent marked valid but that aren't on disk yet, one list per chunk. A list is merged in when its
/// chunk is evaluated. If the lists that can be flushed go over `cap` bytes, the workers freeze and those lists are
/// written to disk.
pub struct PendingMarks<'a> {
    chunks: &'a LayerChunks,
    /// (layout index, combo set) -> IDs of the nodes to mark valid
    marks: DashMap<(usize, Set32), HashSet<PartitionId>, GxBuildHasher>,
    /// Bytes of each layout's lists
    used: Vec<AtomicUsize>,
    peak: AtomicUsize,
    cap: usize,
    /// The freeze: every task holds it shared, and a flush holds it exclusively
    gate: RwLock<()>,
    freezes: AtomicUsize,
    lists_flushed: AtomicUsize,
    flush_nanos: AtomicUsize,
    /// The layout being evaluated. Its lists are never flushed, since a reader may already hold their chunks.
    current: usize,
    /// (key length, child layout) for each key length a node of `current` can shrink by one
    children: Vec<(u32, usize)>,
}

impl<'a> PendingMarks<'a> {
    pub fn new(chunks: &'a LayerChunks, cap: usize) -> Self {
        Self {
            chunks,
            marks: DashMap::default(),
            used: chunks.layouts.iter().map(|_| AtomicUsize::new(0)).collect(),
            peak: AtomicUsize::new(0),
            cap,
            gate: RwLock::new(()),
            freezes: AtomicUsize::new(0),
            lists_flushed: AtomicUsize::new(0),
            flush_nanos: AtomicUsize::new(0),
            current: 0,
            children: Vec::new(),
        }
    }

    /// Call before evaluating `current`
    pub fn start_layout(&mut self, current: usize) {
        let chunks = self.chunks;
        let layout = &chunks.layouts[current];
        self.current = current;
        self.children = get_predecessor_layouts(layout)
            .into_iter()
            // Shrinking a 2-key drops it, which leaves the layer
            .filter(|(i, _)| layout[*i] > 2)
            .filter_map(|(i, child)| {
                let child = chunks.layouts.iter().position(|other| *other == child)?;
                Some((layout[i] as u32, child))
            })
            .collect();
    }

    /// Removes and returns the marks for a chunk of `current`
    pub fn take(&self, combo_set: Set32) -> HashSet<PartitionId> {
        let Some((_, ids)) = self.marks.remove(&(self.current, combo_set)) else {
            return HashSet::default();
        };
        self.used[self.current].fetch_sub(ids.len() * MARK_BYTES, Ordering::Relaxed);
        ids
    }

    /// Marks every same-layer child of a valid node of `current`
    pub fn mark_children(&self, path: &KeyPath) {
        for (key_len, combo_set, id) in child_marks(path) {
            let Some(&(_, child)) = self.children.iter().find(|(len, _)| *len == key_len) else {
                continue;
            };
            // A child can have many valid parents, but its ID is only stored once
            if self.marks.entry((child, combo_set)).or_default().insert(id) {
                self.used[child].fetch_add(MARK_BYTES, Ordering::Relaxed);
            }
        }
    }

    /// Runs one task of `current`, then freezes everything to flush if the cap was crossed. Every `take` and
    /// `mark_children` call must happen inside `f`.
    pub fn run_task(&self, f: impl FnOnce()) {
        {
            // Taken once per task and never nested: a parking_lot read lock can deadlock if nested while a flush
            // is waiting
            let _running = self.gate.read();
            f();
        }
        self.peak.fetch_max(self.total(), Ordering::Relaxed);
        if self.flushable() > self.cap {
            // Waits for the other workers to finish their current task
            let _frozen = self.gate.write();
            // Another worker may have flushed already
            if self.flushable() > self.cap {
                self.flush();
            }
        }
    }

    fn total(&self) -> usize {
        self.used
            .iter()
            .map(|used| used.load(Ordering::Relaxed))
            .sum()
    }

    /// Bytes of the lists that can be flushed. The current layout's lists don't count: they can't be flushed and
    /// only shrink while it runs, so counting them could freeze every task without freeing anything.
    fn flushable(&self) -> usize {
        self.total()
            .saturating_sub(self.used[self.current].load(Ordering::Relaxed))
    }

    /// Writes every list except the current layout's to disk. Only call while holding the gate exclusively. A mark
    /// that arrives later for a flushed chunk starts a new list.
    fn flush(&self) {
        let start = Instant::now();
        let keys: Vec<(usize, Set32)> = self
            .marks
            .iter()
            .map(|entry| *entry.key())
            .filter(|&(layout, _)| layout != self.current)
            .collect();
        let mut by_layout: HashMap<usize, Vec<(Set32, HashSet<PartitionId>)>> = HashMap::new();
        for key in keys {
            let ((layout, combo_set), ids) = self.marks.remove(&key).unwrap();
            self.used[layout].fetch_sub(ids.len() * MARK_BYTES, Ordering::Relaxed);
            by_layout.entry(layout).or_default().push((combo_set, ids));
        }
        for (layout, lists) in by_layout {
            let file = self.chunks.open(layout);
            lists
                .par_iter()
                .for_each(|(combo_set, ids)| self.chunks.apply(&file, layout, *combo_set, ids));
            self.lists_flushed.fetch_add(lists.len(), Ordering::Relaxed);
        }
        self.freezes.fetch_add(1, Ordering::Relaxed);
        self.flush_nanos
            .fetch_add(start.elapsed().as_nanos() as usize, Ordering::Relaxed);
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    pub fn stats(&self) -> MarksStats {
        MarksStats {
            pending_bytes: self.total(),
            peak_bytes: self.peak.load(Ordering::Relaxed),
            freezes: self.freezes.load(Ordering::Relaxed),
            lists_flushed: self.lists_flushed.load(Ordering::Relaxed),
            flush_time: Duration::from_nanos(self.flush_nanos.load(Ordering::Relaxed) as u64),
        }
    }
}
