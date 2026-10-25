//! Where the hosts' Java sources are, for the tests and benches that
//! compile them: the API's (`java-api`) or the host's (`java-common`).
//! Self-contained so a bench can include it as-is.

use std::path::{Path, PathBuf};

/// A host's Java source by its path under `dev/yog` (`bus/YogTypes.java`):
/// the API's when it's there, else the host's.
pub fn host_source(rel: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let api = root.join("java-api/src/main/java/dev/yog").join(rel);
    if api.exists() {
        api
    } else {
        root.join("java-common/src/main/java/dev/yog").join(rel)
    }
}
