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

## Montage

A `.yvideo`'s `montage` (`yog_formats::video::Montage`) lays videos and
pictures on picture tracks — a higher track over a lower, each clip cut
(`from`/`to`), sped up, faded, made see-through and fitted into the
video's frame (its proportions kept) — and its sound as a `.ysound`'s
`Mix`, the videos' own sound laid in on a track of its own (each clip's
`gain`). The montage's size is the `.yvideo`'s `width` and `height` (both
needed), its rate `fps` (30 by default). The build renders it with one
ffmpeg graph: a black frame its length, each shown clip overlaid at its
time.

## Build

ffmpeg renders the `.yvideo` (its source, or its montage's graph) twice:

- the picture as **AV1** (SVT-AV1), packets cut from an IVF stream, kept in
  the video's definition (`yog_registry::VideoDef`: size, frame rate, frame
  count, which frames are keyframes, the packets as one blob and their
  sizes). Sizes are even (4:2:0); a picture smaller than SVT-AV1's
  least (64×64) is padded at the right and bottom, its definition keeping
  the size shown (the top-left part);
- the sound as **Ogg Vorbis**, a sound event `<mod>:video.<name>` in the
  mod's `sounds.json` (streamed).

Why AV1: there's no VP9 decoder in Rust, and libvpx (C) would have to be
cross-built for every platform; rav1d (dav1d ported to Rust) builds with
cargo for all five, and AV1 packs smaller.

## Runtime

`yog_media::video` decodes packets with rav1d into RGBA frames (YUV 4:2:0
→ RGB, BT.709 limited range, as the build encodes). Playing is pure logic
on the definition (`yog_registry::{Clock, Plan}`) — play, pause, seek
(from the keyframe before), loop — so the game and the editors play alike;
yog-api, which mods build against, stays free of the decoder. The runtime answers the `yog:video/…` services: a player's frame is
decoded when it's asked for (each frame it's drawn, the clock moved on by
the time passed), into a texture `yog:textures/dynamic/video_<n>.png`
refilled in place (the hosts' `updateTexture`: one copy into the image,
one upload); mods draw that texture as
any other — a screen's `<Video>`, a quad in the world, an entity's or a
model's texture — and the sound event plays with it (at a place in the
world, or not).

The sound plays from wherever the video is: each play, seek and loop
round starts it again from there. The game can't start a sound partway by
itself, so the hosts do: a mod's sound played `from` some seconds marks
the stream the game opens for it (`YogSounds`), and a mixin on
`SoundBufferLibrary.getStream` reads that much and drops it.

## API

- `yog_api::video::Player::open("mod:intro")` (client): `play`, `pause`,
  `seek`, and `frame()` — the texture to draw (an image source), its size,
  where it is; dropped, it's closed.
- An image source `video:mod:intro`: drawn anywhere a picture is (`<Image>`,
  a background, `draw2d.image`), it plays from its start when first drawn
  and its player goes once it's no longer drawn. `<Video source="intro"
  stretch="Uniform"/>` in `.yui` is that, the mod's own video by default;
  the screen designer shows its first frame.
- `yog_api::video::cutscene("mod:intro", skippable)`: the runtime's screen
  `yog:cutscene`, the video letterboxed on black over the game (modal, not
  paused — a paused game pauses its sounds); it closes at the end, Esc
  skips it when allowed.
- In the world: a player's `frame().texture` is a texture like any other,
  for the mod's own quads. Entities' and models' textures naming a video
  (hosts asking the runtime for its frame as they bind it) — later.

## Stages

1. Names, formats, the definition (`yog-ids`, `yog-formats`, `yog-registry`).
2. The build: `.yres` videos, `.yvideo` compiled — a source, or a montage.
3. The decoder (`yog-media`) and the clock (`yog-registry`), tested on built videos.
4. The runtime's players, the texture, the sound; the API.
5. `<Video>` and `video:` images, cutscenes (done); entities' and models' textures.
6. Yog-IDLE: a video designer — player, the DAW with video tracks.
