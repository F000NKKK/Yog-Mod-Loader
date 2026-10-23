# Video

A mod's videos: made in Yog-IDLE (a montage, as sounds are), compiled by the
build, played by the runtime anywhere the game draws — screens, the world,
entities and models, full-screen cutscenes.

## Files

- **Sources** are `.yres` entries of type `video`: the bytes of any file
  ffmpeg reads (`"@bytes:<base64>"`), shown by the IDE as they are.
- **`.yvideo`** (`resources/video/<name>.yvideo`, id `<mod>:<name>`, path
  `/` as `.`) says what the game shows: a `.yres` video, or a montage of
  video, picture and sound clips on tracks (the sound designer's DAW, with
  video tracks); its size, frame rate and quality; whether it loops and keeps
  its sound.

## Build

ffmpeg renders the `.yvideo` (its source, or its montage's graph) twice:

- the picture as **AV1** (SVT-AV1), packets cut from an IVF stream, kept in
  the video's definition (`yog_registry::VideoDef`: size, frame rate, frame
  count, which frames are keyframes, the packets as one blob and their
  sizes);
- the sound as **Ogg Vorbis**, a sound event `<mod>:video.<name>` in the
  mod's `sounds.json` (streamed).

Why AV1: there's no VP9 decoder in Rust, and libvpx (C) would have to be
cross-built for every platform; rav1d (dav1d ported to Rust) builds with
cargo for all five, and AV1 packs smaller.

## Runtime

`yog-video` decodes a definition's packets with rav1d into RGBA frames
(YUV 4:2:0 → RGB, BT.709), a player keeping its own clock: play, pause,
seek (from the keyframe before), loop. The runtime uploads the frame it's
on into a texture each client frame it changed; mods draw that texture as
any other — a screen's `<Video>`, a quad in the world, an entity's or a
model's texture — and the sound event plays with it (at a place in the
world, or not).

## API

- `yog_api::video::Player::open("mod:intro")` (client): `play`, `pause`,
  `seek`, `looping`, `volume`, `at(pos)` for a sound in the world,
  `texture()` to draw it by.
- `<Video src="@video:mod:intro" autoplay loop/>` in `.yui`.
- `yog_api::video::cutscene("mod:intro", skippable)`: full screen over the
  game, Esc skips it when allowed.

## Stages

1. Names, formats, the definition (`yog-ids`, `yog-formats`, `yog-registry`).
2. The build: `.yres` videos, `.yvideo` compiled (source first, montage after).
3. `yog-video`: the decoder and the player, tested on built videos.
4. The runtime's players, the texture, the sound; the API.
5. `<Video>`, cutscenes, the world, entities and models.
6. Yog-IDLE: a video designer — player, the DAW with video tracks.
