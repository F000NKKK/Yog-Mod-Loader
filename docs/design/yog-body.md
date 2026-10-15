# yog-body — bodies, morphs, camera, movement

One mechanism for everything a mod does with how an entity looks and how
the local player sees and moves — instead of a mixin per feature (wings,
glide, a pose…). The hosts have four generic hooks; the rest is Rust in
the mod (`yog-body`, re-exported as `yog_api::body`).

## What a mod can do

- **Pose the vanilla model**: turn, move, scale, hide any of its parts
  (`head`, `body`, `right_arm`…), on top of vanilla's animation or instead.
- **Turn the whole body** (`root`): a matrix in entity space — lying down,
  gliding, spinning.
- **Add geometry**: its own models (`.ymodel`, skeletons of bones with
  cubes and locators) with its own animations (`.yanim`), on a vanilla part
  (`bone` anchor — wings on the body), on the model, or in entity space.
- **Be something else** (`disguise`): the entity is drawn as another
  entity type, animated live like it (walking, swinging, crouching).
- **Morph**: `capture` asks the host for the posed geometry of the entity
  itself or of another type (every quad, with its texture); next frame it
  arrives in the frame, and `body::morph` blends one into the other, cube by
  cube — the player turns into a zombie before your eyes, like the old
  Morph mod.
- **Hide the original** (`hide`) — full model replacement.
- **Camera**: move, turn, roll the camera, scale the FOV — e.g. follow a
  locator of the player's skeleton.
- **Movement**: replace the local player's travel step (the velocity before
  the move) — gliding, swimming in air, dashes.

## Flow

```
client_body_track(uuid | type)       which entities ask the mods
LivingEntityRenderer.render HEAD  →  nativeBodyPose(frame) → ops   (once per frame per tracked entity)
  root matrix, disguise, ENTITY meshes, hide (cancel)
  after setupAnim: bone ops on the vanilla parts
  after the model: BONE / MODEL meshes
  RETURN: parts restored
Camera.setup TAIL                  →  nativeCamera(frame) → offset/turn/roll/fov
LivingEntity.travel HEAD (local)   →  nativeTravel(frame) → velocity or vanilla
```

Every mod with an `on_body` handler is asked; their ops add up in order.

## Spaces

- **entity**: blocks, y up, origin at the feet, after the body's yaw
  (vanilla `setupRotations`): the model's front is −z.
- **model**: pixels, y down, `ModelPart` space (origin 24 px above the feet).
- **bone `name`**: model space of that vanilla part (after its pose).

Captures come in entity space.

## Wire (little-endian; `str` = u16 length + UTF-8)

Frame: `u8 1`, `str uuid`, `str type`, f32 partial, age, limb_swing,
limb_amount, head_yaw, head_pitch, body_yaw, pitch, f64 x y z, f32 vx vy vz,
u32 flags, u16 captures × (`str source`, u16 meshes × (`str texture`,
u32 vertices × 8 f32: x y z u v nx ny nz)).

Ops, until the end: `1` root (16 f32, column-major), `2` bone (`str name`,
u8 flags: 1 rot set, 2 pos set, 4 scale, 8 hide, 16 show; f32 rot xyz rad,
pos xyz px, scale xyz), `3` mesh (u8 anchor 0 entity 1 model 2 bone,
`str bone`, `str texture`, u8 mode 0 cutout 1 translucent 2 emissive, u8
glint, u32 argb, u32 vertices × 8 f32 — quads), `4` disguise (`str type`),
`5` hide, `6` capture (`str source`: `self` or an entity type).

## Formats

`.ymodel` — Java model space (as `CubeListBuilder`/`PartPose`, what
Blockbench's "Modded Entity" exports):

```json
{
  "texture": "misc/wings.png",       // under textures/, or "ns:path"
  "texture_size": [64, 32],
  "bones": [
    { "name": "left_wing", "parent": null, "pivot": [5, 0, 0], "rotation": [15, 0, -15],
      "cubes": [ { "origin": [-10, 0, 0], "size": [10, 20, 2], "uv": [22, 0], "inflate": 1 } ],
      "locators": { "tip": [-10, 20, 1] } }
  ]
}
```

`.yanim` — clips of keyframes per bone (degrees, pixels), Blockbench-like:

```json
{
  "spread": { "length": 0.5, "loop": "hold",
    "bones": { "left_wing": { "rotation": { "0": [15, 0, -15], "0.5": [20, 0, -90] } } } }
}
```

A clip's bones that the model lacks act on the vanilla model's parts.

## Blocks (planned — for porting Create)

The same rigs, clips and ops drive **block bodies**: a block (entity) asks
the mods each frame like an entity does, with the `block` anchor (pixels
0–16, y up, as block models). That covers Create's kinetic parts — shafts
and cogs turning at a speed (a manual bone turn per frame), pistons and
arms (clips), belts (scrolling UVs). Next steps for it:

- a block frame (dimension, position, state, the block entity's data) and a
  host hook in the block-entity renderer, plus instancing for many parts;
- captures of block-state models (`block:<state>`) — Create's partial
  models and contraptions (a structure drawn as one moving body);
- element models (`.yblock` models) baked into quads, next to `.ymodel`.

## Next: collision and entities

- **Dimensions**: an entity's hitbox and eye height set per entity
  (`entity_set_dimensions`, server and client — one generic hook on
  `getDimensions`), so a morph into a chicken fits through a chicken's gap.
- **Entities of a mod's own**: entity types registered from Rust, drawn
  with rigs (`on_body` with the `entity` anchor and `hide`), with AI and
  sync hooks.
