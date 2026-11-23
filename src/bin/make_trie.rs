use t9_project::evaluation::make_trie::{make_trie, trie_path_for_dict};

// RUSTFLAGS="-C target-cpu=native" cargo run --release --bin make_trie -- word_freq_20000.json
fn main() {
    let dict = std::env::args()
        .nth(1)
        .expect("usage: make_trie <dictionary.json>");
    make_trie(&dict);
    println!("Done: {}", trie_path_for_dict(&dict));
}
