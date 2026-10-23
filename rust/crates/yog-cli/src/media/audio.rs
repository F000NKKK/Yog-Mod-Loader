//! Sounds as the game plays them: Ogg Vorbis. A `.yres` sound may be any
//! format ffmpeg reads (wav, mp3, flac, …); the build makes it Ogg Vorbis,
//! an Ogg file kept as it is.

use crate::media::ffmpeg::{INPUT, OGG_OUT, Scratch, ffmpeg, run};

/// `bytes` as Ogg Vorbis: as they are when they're Ogg already, else converted by ffmpeg.
pub(crate) fn to_ogg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.starts_with(yog_formats::yres::OGG_MAGIC) {
        return Ok(bytes.to_vec());
    }
    let dir = Scratch::new()?;
    let input = dir.input(INPUT, bytes)?;
    run(
        ffmpeg().arg("-i").arg(input).arg("-vn").args(OGG_OUT),
        "convert it to Ogg Vorbis",
    )
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
