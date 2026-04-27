use std::fs;
use std::path::Path;

/// Compile-time validation and code generation for word lists.
///
/// Reads the four categorized word list files from `src/`, verifies their
/// invariants (count, sort order, character set), and writes a generated
/// `wordlists.rs` to `OUT_DIR` containing static `&[&str]` slices.
#[allow(clippy::panic)]
fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("wordlists.rs");

    let entries: &[(&Path, &str, usize)] = &[
        (Path::new("src/determiners.txt"), "DETERMINERS", 64),
        (Path::new("src/adjectives.txt"), "ADJECTIVES", 2048),
        (Path::new("src/nouns.txt"), "NOUNS", 1024),
        (Path::new("src/verbs.txt"), "VERBS", 1024),
    ];

    let mut code = String::new();

    for (path, static_name, expected_count) in entries {
        let contents = fs::read_to_string(path).unwrap_or_else(|_| panic!("reading {path:?}"));

        let lines: Vec<&str> = contents
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();

        assert_eq!(
            lines.len(),
            *expected_count,
            "{path:?}: expected {expected_count} words, got {}",
            lines.len()
        );

        // Verify invariants
        for pair in lines.windows(2) {
            assert!(
                pair[0] < pair[1],
                "{path:?}: not sorted at {:?} / {:?}",
                pair[0],
                pair[1]
            );
        }

        for w in &lines {
            assert!(
                !w.is_empty() && w.len() <= 12 && w.chars().all(|c| c.is_ascii_lowercase()),
                "{path:?}: invalid word {w:?} (must be 1-12 ascii lowercase)"
            );
        }

        // Generate a static slice. We emit a &[&str; N] array reference so
        // binary_search works directly.
        let words_literal = lines
            .iter()
            .map(|w| format!("\"{w}\""))
            .collect::<Vec<_>>()
            .join(",\n        ");

        code.push_str(&format!(
            "#[allow(dead_code)]\npub(crate) static {static_name}: &[&str] = &[\n        {words_literal}\n    ];\n\n"
        ));
    }

    fs::write(&dest, code).expect("failed to write wordlists.rs");
    println!("cargo:rerun-if-changed=src/determiners.txt");
    println!("cargo:rerun-if-changed=src/adjectives.txt");
    println!("cargo:rerun-if-changed=src/nouns.txt");
    println!("cargo:rerun-if-changed=src/verbs.txt");
}
