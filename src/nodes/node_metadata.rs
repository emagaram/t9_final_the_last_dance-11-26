use std::{
    fs::{rename, File},
    io::{BufReader, BufWriter},
    path::Path,
};

use bincode::{config, Decode, Encode};
use gxhash::{HashMap, HashMapExt};

use crate::general::set32::Set32;

#[derive(Decode, Encode, Clone, Debug, Copy)]
pub enum NodeType {
    Standard,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Standard => write!(f, "standard"),
        }
    }
}

#[derive(Decode, Encode, Clone)]
pub struct NodeMetadataItem {
    pub start: usize,
    pub len_bytes: usize,
}

impl NodeMetadataItem {
    pub fn new(start: usize, len_bytes: usize) -> Self {
        NodeMetadataItem { start, len_bytes }
    }
}

#[derive(Decode, Encode, Clone)]
pub struct NodeMetadata {
    pub layout: Vec<usize>,
    pub items: HashMap<Set32, NodeMetadataItem>,
    pub node_type: NodeType,
}
impl NodeMetadata {
    pub fn new(node_type: NodeType, layout: &[usize]) -> Self {
        NodeMetadata {
            layout: layout.to_vec(),
            items: HashMap::new(),
            node_type,
        }
    }
    fn get_metadata_path(directory: &str, layout_config: &[usize], node_type: NodeType) -> String {
        let layout_str = match layout_config.is_empty() {
            true => "root".to_string(),
            false => layout_config
                .iter()
                .map(|&len| len.to_string())
                .collect::<Vec<String>>()
                .join("_"),
        };

        format!(
            "{}/L{}/{}/{}/metadata.bin",
            directory,
            layout_config.len(),
            node_type,
            layout_str
        )
    }

    pub fn get_node_metadata(
        directory: &str,
        layout_config: &[usize],
        node_type: NodeType,
    ) -> Option<NodeMetadata> {
        let path = Self::get_metadata_path(directory, layout_config, node_type);
        let file = File::open(path);
        if file.is_err() {
            return None; // File not found
        }
        let file = file.unwrap();
        let mut reader = BufReader::new(file);

        let node_metadata: NodeMetadata =
            bincode::decode_from_std_read(&mut reader, config::standard())
                .expect("Failed to decode solution node metadata");
        Some(node_metadata)
    }
    pub fn write_node_metadata(directory: &str, metadata: &NodeMetadata) -> usize {
        let final_path_str =
            Self::get_metadata_path(directory, &metadata.layout, metadata.node_type.clone());
        let final_path = Path::new(&final_path_str);
        if !final_path.is_dir() {
            std::fs::create_dir_all(final_path.parent().unwrap())
                .expect("Unable to create scorecasts directory");
        }
        let file = tempfile::Builder::new()
            .tempfile_in(final_path.parent().unwrap())
            .expect("Unable to create temp file");
        let mut writer = BufWriter::new(&file);

        let node_metadata =
            bincode::encode_into_std_write(metadata, &mut writer, config::standard())
                .expect("Failed to decode solution node metadata");
        file.as_file().sync_all().expect("Unable to sync temp file");

        rename(file.path(), final_path_str).expect("Unable to rename temp file to metadata file");
        node_metadata
    }
}
