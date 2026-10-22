//! Sounds as the game plays them: Ogg Vorbis. A `.yres` sound may be any
//! format ffmpeg reads (wav, mp3, flac, …); the build makes it Ogg Vorbis,
//! an Ogg file kept as it is.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// The program that converts, as found on `PATH`.
const FFMPEG: &str = "ffmpeg";
/// Its arguments: quiet, the input on stdin, Ogg Vorbis at a good quality on stdout.
const ARGS: &[&str] = &[
    "-hide_banner",
    "-loglevel",
    "error",
    "-i",
    "pipe:0",
    "-vn",
    "-c:a",
    "libvorbis",
    "-q:a",
    "6",
    "-f",
    "ogg",
    "pipe:1",
];

/// `bytes` as Ogg Vorbis: as they are when they're Ogg already, else converted by ffmpeg.
pub(crate) fn to_ogg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.starts_with(yog_formats::yres::OGG_MAGIC) {
        return Ok(bytes.to_vec());
    }
    let mut child = Command::new(FFMPEG)
        .args(ARGS)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!("not Ogg Vorbis, and {FFMPEG} (which converts it) can't be run: {e}")
        })?;
    let mut stdin = child.stdin.take().ok_or("ffmpeg's input")?;
    let input = bytes.to_vec();
    // Written from another thread: ffmpeg's output would fill its pipe meanwhile.
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let mut out = Vec::new();
    child
        .stdout
        .take()
        .ok_or("ffmpeg's output")?
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    let mut err = String::new();
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut err);
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let written = writer.join().map_err(|_| "writing to ffmpeg panicked")?;
    if !status.success() || out.is_empty() {
        return Err(format!("{FFMPEG} couldn't convert it: {}", err.trim()));
    }
    written.map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ogg_file_is_kept_as_it_is() {
        let ogg = [yog_formats::yres::OGG_MAGIC, b"rest"].concat();
        assert_eq!(to_ogg(&ogg).unwrap(), ogg);
    }
}
