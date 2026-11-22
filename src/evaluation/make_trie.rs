use std::path::Path;

use crate::{
    evaluation::final_evaluate::{
        scores_hm_tree::{
            gen_hm_dyn_tree_new, gen_hm_static_tree32, gen_hm_vec_new, save_stream32,
        },
        scores_hm_tree_node::StaticTrie32,
    },
    general::set32::Set32,
};
use gxhash::HashMap;

/// Derives the trie output path from the dictionary path, matching the
/// existing naming convention: `word_freq_20000.json` -> `hm_trie32_20000.bin`.
pub fn trie_path_for_dict(dict: &str) -> String {
    let stem = Path::new(dict)
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("dictionary path has no file name");
    let suffix = stem.strip_prefix("word_freq_").unwrap_or(stem);
    format!("hm_trie32_{}.bin", suffix)
}

/// Builds the static criteria trie (`hm_static_tree`) for `dict` and saves it
/// to `trie_path_for_dict(dict)` in the cwd, where the solver's `HM_TRIE` expects it.
/// Run with `cargo run --release --bin make_trie -- DICT.json`
pub fn make_trie(dict: &str) -> HashMap<Set32, StaticTrie32> {
    let hm_vec = gen_hm_vec_new(dict);
    let hm_dyn_tree = gen_hm_dyn_tree_new(hm_vec);
    let hm_static_tree = gen_hm_static_tree32(hm_dyn_tree);

    let out_path = trie_path_for_dict(dict);
    println!("Saving trie to {}", out_path);
    save_stream32(&hm_static_tree, &out_path);
    println!("Trie saved");
    hm_static_tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::final_evaluate::scores_hm_tree::read_stream32;

    #[test]
    fn test_trie_path_for_dict() {
        assert_eq!(
            trie_path_for_dict("word_freq_20000.json"),
            "hm_trie32_20000.bin"
        );
        assert_eq!(
            trie_path_for_dict("word_freq_full.json"),
            "hm_trie32_full.bin"
        );
        assert_eq!(trie_path_for_dict("other.json"), "hm_trie32_other.bin");
    }

    // RUSTFLAGS="-Awarnings" cargo test --release test_make_trie_round_trip -- --nocapture
    #[test]
    fn test_make_trie_round_trip() {
        let dict = "word_freq_2000.json";
        let hm_static_tree = make_trie(dict);
        let out_path = trie_path_for_dict(dict);

        let loaded_tree = read_stream32(&out_path).expect("Failed to read the static tree");
        assert_eq!(hm_static_tree.len(), loaded_tree.len());
        for (key, static_trie) in &hm_static_tree {
            let loaded_trie = loaded_tree.get(key).expect("Key not found in loaded tree");
            assert_eq!(static_trie.nodes.len(), loaded_trie.nodes.len());
            assert_eq!(static_trie.edges.len(), loaded_trie.edges.len());
        }
        std::fs::remove_file(&out_path).unwrap();
    }
}
