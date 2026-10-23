//! The toolkit Yog's analyzers are written with: the sources under a folder
//! (and a fingerprint that changes when any of them does), and a report that
//! places each problem found in a source as a `yog_data::Diagnostic`.

pub mod files;
mod report;

pub use report::Report;
