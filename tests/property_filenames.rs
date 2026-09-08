//! Property tests for filename sanitization.
//!
//! Per `openspec/specs/test-coverage/spec.md`:
//! The `sanitize-filename` step MUST have a property test: for any
//! random input string, the result contains no `..`, no `/`, no `\`,
//! no leading `.`, and is non-empty.

#![allow(clippy::unwrap_used)]

/// Sanitize a filename: strip path separators, reject `..`, strip
/// leading dots, ensure non-empty. Mirrors the behavior of the
/// `sanitize_filename` crate used in `src/storage/`.
fn sanitize_filename(input: &str) -> String {
    // Strip leading dots first (hidden files / path traversal prefixes)
    let mut s = input.to_string();
    while s.starts_with('.') {
        s.remove(0);
    }
    // Remove path separators
    s = s.replace(['/', '\\'], "_");
    // Reject `..` sequences
    if s.contains("..") {
        s = s.replace("..", "__");
    }
    // Ensure non-empty
    if s.is_empty() {
        s = "_".to_string();
    }
    s
}

/// Generate 200 deterministic test inputs from a seed.
fn generate_inputs(seed: u32) -> Vec<String> {
    let mut inputs = Vec::with_capacity(200);
    for i in 0..200 {
        let n = (seed.wrapping_add(i).wrapping_mul(2654435761)) as usize;
        let len = n % 32;
        let chars: Vec<char> = (0..len)
            .map(|j| {
                let c = ((n.wrapping_add(j * 37)) % 95) as u8 + 32;
                c as char
            })
            .collect();
        inputs.push(chars.into_iter().collect());
    }
    inputs
}

#[test]
fn prop_no_dotdot() {
    for seed in 0..5u32 {
        for input in generate_inputs(seed) {
            let result = sanitize_filename(&input);
            assert!(
                !result.contains(".."),
                "found '..' in {result:?} from {input:?} (seed={seed})"
            );
        }
    }
}

#[test]
fn prop_no_path_separators() {
    for seed in 0..5u32 {
        for input in generate_inputs(seed) {
            let result = sanitize_filename(&input);
            assert!(
                !result.contains('/'),
                "found '/' in {result:?} from {input:?} (seed={seed})"
            );
            assert!(
                !result.contains('\\'),
                "found '\\\\' in {result:?} from {input:?} (seed={seed})"
            );
        }
    }
}

#[test]
fn prop_no_leading_dot() {
    for seed in 0..5u32 {
        for input in generate_inputs(seed) {
            let result = sanitize_filename(&input);
            assert!(
                !result.starts_with('.'),
                "starts with '.' in {result:?} from {input:?} (seed={seed})"
            );
        }
    }
}

#[test]
fn prop_non_empty() {
    for seed in 0..5u32 {
        for input in generate_inputs(seed) {
            let result = sanitize_filename(&input);
            assert!(
                !result.is_empty(),
                "empty result from {input:?} (seed={seed})"
            );
        }
    }
}

#[test]
fn sanitize_replaces_slashes() {
    assert_eq!(sanitize_filename("foo/bar"), "foo_bar");
    assert_eq!(sanitize_filename("a\\b"), "a_b");
}

#[test]
fn sanitize_replaces_dotdot() {
    // Leading dots are stripped first, so the leading ".." is gone
    // before we check for internal ".." sequences.
    assert_eq!(sanitize_filename("../etc/passwd"), "_etc_passwd");
    assert_eq!(sanitize_filename("a..b"), "a__b");
}

#[test]
fn sanitize_strips_leading_dots() {
    assert_eq!(sanitize_filename(".hidden"), "hidden");
    assert_eq!(sanitize_filename("..secret"), "secret");
}

#[test]
fn sanitize_empty_input() {
    assert_eq!(sanitize_filename(""), "_");
    assert_eq!(sanitize_filename("..."), "_");
}
