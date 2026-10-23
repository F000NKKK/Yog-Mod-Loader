//! JNI: the runtime's entry points the hosts call (`Java_dev_yog_*`) and
//! its calls into the hosts' `dev.yog.*` methods.

pub(crate) mod call;
mod client;
mod init;
mod server;
pub(crate) mod ui;
pub(crate) mod util;
