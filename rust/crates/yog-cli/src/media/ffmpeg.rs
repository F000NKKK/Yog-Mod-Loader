//! ffmpeg (and ffprobe), as found on `PATH`: what the build renders sounds
//! and videos with. Inputs go through files of a scratch folder of their
//! own; outputs come back on stdout.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// The programs, as found on `PATH`.
pub(crate) const FFMPEG: &str = "ffmpeg";
pub(crate) const FFPROBE: &str = "ffprobe";
/// The first arguments of every ffmpeg run: no banner, errors alone.
pub(crate) const QUIET: &[&str] = &["-hide_banner", "-loglevel", "error"];
/// ffprobe's: errors alone.
pub(crate) const PROBE_QUIET: &[&str] = &["-v", "error"];
/// Sound written as the game plays it: Ogg Vorbis at a good quality, on stdout.
pub(crate) const OGG_OUT: &[&str] = &["-c:a", "libvorbis", "-q:a", "6", "-f", "ogg", STDOUT];
/// The name of a run's one input in its scratch folder.
pub(crate) const INPUT: &str = "in";
/// ffmpeg's name for its standard output.
pub(crate) const STDOUT: &str = "pipe:1";

/// A folder of its own for a run's inputs, gone when dropped.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Result<Scratch, String> {
        static NEXT: yog_sync::atomic::AtomicU64 = yog_sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, yog_sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("yog-ffmpeg-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Scratch(dir))
    }

    /// `bytes` written as its file `name`: the file's path.
    pub(crate) fn input(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// ffmpeg, quiet, ready for arguments.
pub(crate) fn ffmpeg() -> Command {
    let mut cmd = Command::new(FFMPEG);
    cmd.args(QUIET);
    cmd
}

/// ffprobe, quiet, ready for arguments.
pub(crate) fn ffprobe() -> Command {
    let mut cmd = Command::new(FFPROBE);
    cmd.args(PROBE_QUIET);
    cmd
}

/// `cmd` run (`what`: what it does, for its errors): what it wrote out.
pub(crate) fn run(cmd: &mut Command, what: &str) -> Result<Vec<u8>, String> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{what} needs {program}, which can't be run: {e}"))?;
    if !out.status.success() || out.stdout.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{program} couldn't {what}: {}", err.trim()));
    }
    Ok(out.stdout)
}
