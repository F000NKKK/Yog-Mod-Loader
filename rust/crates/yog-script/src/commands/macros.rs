//! `--macros <file.ys>` and `--to-server` before a command's files: the
//! procedural macros the files are checked with, and what their mod says.

use yog_script_build::MacroLibrary;

/// The flag naming a macros' source.
const FLAG: &str = "--macros";
/// The flag saying the sources' mod sends to the server (its `yog.toml`'s
/// `[network] to_server`).
const TO_SERVER: &str = "--to-server";

/// What a command's arguments say besides its files.
pub(super) struct Given {
    pub macros: Option<MacroLibrary>,
    pub to_server: bool,
    pub files: Vec<String>,
    /// What the files include, beside them (named from where the tool runs).
    pub beside: yog_script_build::FilesBeside,
}

impl Given {
    /// The project the files are checked in.
    pub fn project(&self) -> yog_script_build::Project<'_> {
        yog_script_build::Project {
            macros: self.macros.as_ref(),
            files: Some(&self.beside),
            to_server: self.to_server,
        }
    }
}

/// `rest` split into the macros' library (when any is named), the flags and the files.
pub(super) fn split(rest: &[String]) -> Result<Given, String> {
    let mut sources = Vec::new();
    let mut files = Vec::new();
    let mut to_server = false;
    let mut args = rest.iter();
    while let Some(a) = args.next() {
        if a == TO_SERVER {
            to_server = true;
        } else if a == FLAG {
            let path = args.next().ok_or_else(|| format!("{FLAG} takes a file"))?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            sources.push((path.clone(), text));
        } else {
            files.push(a.clone());
        }
    }
    if sources.is_empty() {
        return Ok(Given {
            macros: None,
            to_server,
            files,
            beside: here(),
        });
    }
    let lib = MacroLibrary::load(&sources).map_err(|problems| {
        problems.iter().for_each(|d| eprintln!("{d}"));
        format!("the macros ({}) have errors", sources.len())
    })?;
    Ok(Given {
        macros: Some(lib),
        to_server,
        files,
        beside: here(),
    })
}

/// Files beside the sources, named from where the tool runs.
fn here() -> yog_script_build::FilesBeside {
    yog_script_build::FilesBeside {
        base: std::path::PathBuf::from(CURRENT_DIR),
    }
}

/// The folder the tool runs in.
const CURRENT_DIR: &str = ".";
