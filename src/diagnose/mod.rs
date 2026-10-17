//! `macora diagnose`: collect facts with parallel probes, analyze them into steps and a verdict,
//! and render the result as text, JSON, Markdown or HTML.
pub mod analyze;
pub mod collect;
pub mod facts;
pub mod parse;
pub mod pmtu;
pub mod render;

pub use analyze::Report;
pub use collect::{Options, RealRunner, Runner};
pub use facts::{Facts, Status, Step};

use crate::model::Snapshot;

/// Runs every probe against the live network and analyzes the result.
pub fn run(snapshot: &Snapshot, opts: &Options) -> Report {
    analyze::analyze(&collect::collect(&RealRunner, snapshot, opts))
}
