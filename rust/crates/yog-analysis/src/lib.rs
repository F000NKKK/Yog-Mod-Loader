//! The toolkit Yog's analyzers are written with: the sources under a folder
//! (and a fingerprint that changes when any of them does), a preprocessor
//! for line directives (`#if`), and a report that places each problem found
//! in a source as a `yog_data::Diagnostic`.

pub mod files;
pub mod preprocess;
mod report;

pub use report::Report;
