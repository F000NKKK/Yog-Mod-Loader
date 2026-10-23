//! Records written to the game's log.

use yog_data::Schema;

/// A record a mod wrote (`yog_api::info!`…): the runtime puts it in the
/// game's log; anyone else may listen (a debugger's console).
#[derive(Debug, Clone, Schema)]
pub struct Record {
    /// `yog_logging::Level` as a number.
    pub level: u32,
    pub message: String,
}
