//! What Yog's tools share: environment variables, the names exported to other
//! languages, file formats and where files live, platforms, versions.

pub mod env;
pub mod export;
pub(crate) mod formats;
pub mod platform;
pub mod versions;
