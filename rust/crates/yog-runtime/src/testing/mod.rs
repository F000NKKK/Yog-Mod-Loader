//! What the tests and benchmarks reach: a JVM in this process, the
//! runtime's hot paths, the registration and dispatch tests.

#[cfg(feature = "bench")]
pub mod bench_support;
pub mod jvm;
#[cfg(test)]
pub(crate) mod jvm_shared;
#[cfg(test)]
mod tests;
