// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Snippet ID computation and collision resolution per D-007.
//!
//! Default ID = `base32_lower(blake3(src))[..12]`. Explicit IDs are pinned
//! verbatim. Collisions among default IDs get an occurrence-index suffix;
//! collisions among explicit IDs are a hard error (deferred to caller).

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

pub(crate) fn default_id(src: &str) -> String {
    let h = blake3::hash(src.as_bytes());
    let bytes = h.as_bytes();
    let mut out = String::with_capacity(12);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &b in bytes.iter() {
        acc = (acc << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 && out.len() < 12 {
            bits -= 5;
            let idx = ((acc >> bits) & 0x1f) as usize;
            out.push(ALPHABET[idx] as char);
        }
        if out.len() >= 12 {
            break;
        }
    }
    out
}

pub(crate) fn resolve_collisions(default_ids: &[String]) -> Vec<String> {
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut out = Vec::with_capacity(default_ids.len());
    for id in default_ids {
        let n = counts.entry(id.as_str()).or_insert(0);
        if *n == 0 {
            out.push(id.clone());
        } else {
            out.push(format!("{id}-{n}"));
        }
        *n += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_id_is_12_lowercase_base32() {
        let id = default_id("println!(\"hi\");");
        assert_eq!(id.len(), 12);
        assert!(id.chars().all(|c| ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn default_id_is_deterministic() {
        assert_eq!(default_id("let x = 1;"), default_id("let x = 1;"));
    }

    #[test]
    fn whitespace_changes_id() {
        assert_ne!(default_id("let x=1;"), default_id("let x = 1;"));
    }

    #[test]
    fn collision_suffix_uses_occurrence_index() {
        let ids = vec![
            "abc".to_string(),
            "xyz".to_string(),
            "abc".to_string(),
            "abc".to_string(),
        ];
        assert_eq!(
            resolve_collisions(&ids),
            vec!["abc", "xyz", "abc-1", "abc-2"]
        );
    }
}
