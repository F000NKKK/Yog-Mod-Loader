//! Game versions matched with what a source says of them: `targets mc …;`
//! and `#if mc …` — the one way, compared as Yog compares versions.

use std::cmp::Ordering::{Equal, Greater, Less};

use crate::ast::{BinOp, VersionCond};

/// Whether version `v` meets `cond`.
#[must_use]
pub fn takes(cond: &VersionCond, v: &str) -> bool {
    match cond {
        VersionCond::Range { from, to } => {
            yog_home::version::compare(v, from) != Less
                && yog_home::version::compare(v, to) != Greater
        }
        VersionCond::Compare { op, version } => {
            let ord = yog_home::version::compare(v, version);
            match op {
                BinOp::GtEq => ord != Less,
                BinOp::Gt => ord == Greater,
                BinOp::LtEq => ord != Greater,
                BinOp::Lt => ord == Less,
                _ => ord == Equal,
            }
        }
    }
}

/// The versions Yog runs on that `cond` takes, oldest first.
#[must_use]
pub fn targeted(cond: &VersionCond) -> Vec<&'static str> {
    yog_ids::game_version::ALL_VALUES
        .iter()
        .copied()
        .filter(|v| takes(cond, v))
        .collect()
}

/// The version a source is read as when no version is asked for (an
/// editor's check): the newest Yog runs on.
#[must_use]
pub fn newest() -> &'static str {
    yog_ids::game_version::ALL_VALUES
        .last()
        .copied()
        .unwrap_or_default()
}
