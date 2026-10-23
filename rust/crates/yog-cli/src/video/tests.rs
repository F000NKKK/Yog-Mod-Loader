use super::*;

fn video(text: &str) -> Video {
    yog_data::from_text::<Video>(text).unwrap().value
}

#[test]
fn a_bad_quality_is_refused_before_ffmpeg_runs() {
    let err = render(&video(r#"{ source: "@V.a", quality: 64 }"#), &[]).unwrap_err();
    assert!(err.contains("quality 64"), "{err}");
}

/// A 2-second 24 fps test picture with a tone, as WebM.
pub(crate) fn source() -> Vec<u8> {
    let mut cmd = ffmpeg();
    cmd.args([
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=161x91:rate=24:duration=2",
    ])
    .args(["-f", "lavfi", "-i", "sine=duration=2"])
    .args([
        "-c:v",
        "libvpx-vp9",
        "-c:a",
        "libvorbis",
        "-f",
        "webm",
        STDOUT,
    ]);
    run(&mut cmd, "make a test video").unwrap()
}

#[test]
fn a_video_renders_into_av1_packets_and_a_sound() {
    let src = source();
    let r = render(&video(r#"{ source: "@V.a", fps: 10 }"#), &src).unwrap();
    assert_eq!((r.width, r.height), (160, 90), "made even");
    assert_eq!((r.fps_num, r.fps_den), (10, 1));
    assert_eq!(r.sizes.len(), 20);
    assert_eq!(r.sizes.iter().sum::<u32>() as usize, r.packets.len());
    assert_eq!(r.keyframes[0], 0);
    assert!(
        r.sound
            .as_ref()
            .is_some_and(|s| s.starts_with(yog_formats::yres::OGG_MAGIC))
    );
    let silent = render(&video(r#"{ source: "@V.a", sound: false }"#), &src).unwrap();
    assert!(silent.sound.is_none());
}
