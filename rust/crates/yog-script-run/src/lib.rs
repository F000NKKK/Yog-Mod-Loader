//! Yog Script's interpreter: checked code run over its tree. It runs what
//! the build needs to run (procedural macros, constants), `yog test`'s
//! tests, and is the meaning compiled code must keep.
//!
//! ```
//! let checked = yog_script_analyze::check("a.ys", "i32 Twice(i32 x) => x * 2;\n");
//! let program = yog_script_run::Program::of(&[&yog_script_analyze::system::DECLS, &checked.file.decls]);
//! let mut run = yog_script_run::Interp::new(&program);
//! assert_eq!(run.call("Twice", vec![yog_script_run::Value::Int(21)]), Ok(yog_script_run::Value::Int(42)));
//! ```

mod interp;
mod natives;
mod program;
mod value;

pub use interp::{Interp, Panic};
pub use program::Program;
pub use value::Value;
