//! Names in each language's case, from one splitting into words.

use crate::consts::{SNAKE_JOIN, WORD_SEPARATORS};

/// The words of a name: split at separators (`_ - / . space`) and before
/// each capital (`statBar`, `StatBar`, `stat_bar`, `stat-bar` → `stat`, `bar`
/// as written: `stat`, `Bar`).
pub fn words(name: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for part in name.split(WORD_SEPARATORS).filter(|p| !p.is_empty()) {
        let mut start = 0;
        for (i, c) in part.char_indices().skip(1) {
            if c.is_uppercase() {
                out.push(&part[start..i]);
                start = i;
            }
        }
        out.push(&part[start..]);
    }
    out
}

/// A word with its first letter a capital.
fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

/// `stat_bar` / `stat-bar` / `statBar` → `StatBar`.
pub fn pascal(name: &str) -> String {
    words(name).into_iter().map(capitalized).collect()
}

/// `stat_bar` / `StatBar` → `statBar`.
pub fn camel(name: &str) -> String {
    let pascal = pascal(name);
    let mut chars = pascal.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

/// `StatBar` / `stat-bar` → `stat_bar`.
pub fn snake(name: &str) -> String {
    joined_lower(name).join(SNAKE_JOIN)
}

/// `statBar` / `stat_bar` → `STAT_BAR`.
pub fn screaming(name: &str) -> String {
    snake(name).to_uppercase()
}

fn joined_lower(name: &str) -> Vec<String> {
    words(name).into_iter().map(str::to_lowercase).collect()
}

#[cfg(test)]
mod tests;
