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

/// `r`'s frames `frames` decoded (after a reset when `reset`): each one's
/// top-left pixel.
fn decode(
    d: &mut yog_media::video::Decoder,
    r: &Rendered,
    plan: &yog_registry::Plan,
) -> Vec<[u8; 4]> {
    if plan.reset {
        d.reset();
    }
    let mut at = 0usize;
    let mut out = Vec::new();
    for (i, &n) in r.sizes.iter().enumerate() {
        let packet = &r.packets[at..at + n as usize];
        at += n as usize;
        if !plan.frames.contains(&(i as u32)) {
            continue;
        }
        let mut rgba = Vec::new();
        let size = (r.width as usize, r.height as usize);
        d.decode(packet, |planes, _| {
            yog_media::video::yuv::to_rgba(planes, size, &mut rgba)
        })
        .unwrap()
        .unwrap();
        out.push([rgba[0], rgba[1], rgba[2], rgba[3]]);
    }
    out
}

#[test]
fn rendered_packets_decode_from_the_start_and_from_a_keyframe() {
    let mut cmd = ffmpeg();
    cmd.args([
        "-f",
        "lavfi",
        "-i",
        "color=c=red:size=96x54:rate=10:duration=3",
    ])
    .args(["-c:v", "libvpx-vp9", "-f", "webm", STDOUT]);
    let src = run(&mut cmd, "make a test video").unwrap();
    let r = render(&video(r#"{ source: "@V.a" }"#), &src).unwrap();
    assert_eq!(
        r.keyframes.len(),
        2,
        "about every two seconds: {:?}",
        r.keyframes
    );
    let key = r.keyframes[1];
    let mut d = yog_media::video::Decoder::new().unwrap();
    let all = yog_registry::Plan {
        reset: true,
        frames: 0..30,
    };
    let px = decode(&mut d, &r, &all);
    assert_eq!(px.len(), 30);
    let red = |p: &[u8; 4]| p[0] > 240 && p[1] < 16 && p[2] < 16 && p[3] == 255;
    assert!(px.iter().all(red), "{:?}", px[0]);
    let seek = yog_registry::Plan {
        reset: true,
        frames: key..key + 5,
    };
    assert!(decode(&mut d, &r, &seek).iter().all(red));
}
