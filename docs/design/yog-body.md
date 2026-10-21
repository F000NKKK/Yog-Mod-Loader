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

## Wire (Yog Data, on the bus's `client/render/body` event)

The frame and the ops are `yog_body::wire`'s records — the hosts read and
write them through the Java records generated from them (`YogTypes.BodyFrame`,
`YogTypes.BodyOps`); nothing is laid out by hand on either side.

- **Frame** (`BodyFrame`): uuid, type, partial, age, limb swing and amount,
  head yaw and pitch, body yaw, pitch, position (f64), velocity, flags, the
  game's time; last frame's captures (`BodyCapture`: source, meshes); what
  the entity keeps (`BodyKept`: key, bytes), filled in by the runtime.
- **Ops** (`BodyOps`), every handler's added up (`Ops::add`): `root` (16
  floats, column-major — turns compose), `bones` (`BodyBone`: a vanilla part
  and its `BodyBoneOp` — rotation, position, set or added, scale, shown or
  hidden), `draws` (`BodyDraw`: anchor `entity`/`model`/`bone`/`block`, the
  bone, the mesh, mode `cutout`/`translucent`/`emissive`, glint, ARGB tint),
  `disguise` (an entity type), `hide`, `captures` (`self` or entity types).
- A mesh (`BodyMesh`) is a texture and its quads' vertices, 8 floats each:
  x y z u v nx ny nz, four a quad.

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

## Next: editors and content that uses bodies

- **Yog-IDLE editors** for `.ymodel`/`.yanim` like the `.yblock` designer,
  but in 3D, Blender-like: orbit/pan/zoom, select bones and cubes, move,
  rotate, scale with gizmos, edit pivots and UVs, a timeline with
  keyframes and curves, preview on the player or a mob, morph preview.
- **Content made of bodies**: `.yblock`, `.yitem`, `.yentity` (and mobs)
  reference models and clips (`"model": "@model:mymod:wings"`,
  `"animations": "@anim:mymod:wings"`) or embed them — compiled by
  `yog build`, reusable between content: an item's held model, a block's
  moving parts, an entity's whole body, a mob's animations by state.

## Entity data and the network (ABI 3.2)

`Server::set_entity_data(uuid, key, value, persist)` — the loader's
`SynchedEntityData`/attachments. Rules the whole sync layer follows:

- **The server decides.** Only the server writes state (entity data,
  positions, collisions); a client sends intents in its mod's packets and the
  server checks them. Clients do the heavy, cosmetic work — drawing,
  animation, morph geometry — from the state they're given.
- **Only to who needs it.** A change goes to the players who see the entity
  (and to it, if a player); a player who starts seeing it gets all of it.
- **Once a tick.** Changes are batched per entity and sent at the end of the
  server tick; a key set twice in a tick goes once; unchanged values aren't
  sent.
- **Saved when asked** (`persist`): with the entity's NBT; an unloaded
  entity is forgotten from memory.
- **Shared time.** Body frames carry the world's game time, the same on
  every client, so clips started at a server tick play alike everywhere.

For moving structures and their collisions (Create's contraptions) the same
holds: the server simulates and sends snapshots stamped with the tick;
clients interpolate between snapshots and blend corrections smoothly, so
nothing drifts apart — the server's position wins.
