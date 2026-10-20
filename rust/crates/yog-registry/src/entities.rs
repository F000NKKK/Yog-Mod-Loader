//! Entities: what a mod declares an entity type as.


/// What kind of entity a type is.
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntityKind {
    /// Not alive: a thing in the world (a scroll on a wall, a vehicle, a
    /// display) — no health, no AI.
    #[default]
    Object,
    /// Alive: health, attributes, hurt and death, but no AI of its own.
    Living,
    /// A creature with AI: `goals`, and walking about.
    Mob,
    /// Hangs on a block's side, as a painting: `width`×`height` whole
    /// blocks of wall (the item it's placed from may say another size —
    /// [`ItemDef::hangs`]); it stays while the blocks behind hold it, and
    /// breaking it, or losing its wall, drops the item it was placed from.
    Hanging,
}

/// An entity type to register (`.yentity`). Its look is a body model
/// (`model`, `.ymodel`) with clips by state; what it does beyond `goals` is
/// the mod's code (entity events, a tick of its own).
#[derive(yog_data::Schema, Debug, Clone)]
pub struct EntityDef {
    pub id: String,
    /// Its name (a text or a `@Set.key` resource); default: the id's lang key.
    pub name: Option<String>,
    #[data(default)]
    pub kind: EntityKind,
    /// Its spawn group (`MobCategory`): `misc`, `monster`, `creature`,
    /// `ambient`, `water_creature`, `water_ambient`, `underground_water_creature`.
    #[data(default = "misc")]
    pub category: String,
    /// Hitbox, blocks.
    pub width: f32,
    pub height: f32,
    /// Eye height, blocks; default 85% of the height.
    pub eye_height: Option<f32>,
    /// Lava and fire don't hurt it.
    #[data(default)]
    pub fire_immune: bool,
    /// It doesn't fall.
    #[data(default)]
    pub no_gravity: bool,
    /// Others push it (living things by default).
    pub pushable: Option<bool>,
    /// Others stand on and bump into it as into a block (a boat).
    #[data(default)]
    pub solid: bool,
    /// How far players see it, chunks.
    #[data(default = "eight")]
    pub tracking_range: u32,
    /// How often its movement is sent, ticks.
    #[data(default = "three")]
    pub update_interval: u32,
    /// Living and mob attributes by id: `max_health`, `movement_speed`,
    /// `attack_damage`, `follow_range`, `armor`, `knockback_resistance`… (the
    /// game's, `minecraft:` optional, or a mod's).
    #[data(default)]
    pub attributes: yog_data::Ordered<f64>,
    /// Its body model (`.ymodel` id); none: the mod draws it (`on_body`).
    pub model: Option<String>,
    /// Its clips (`.yanim` id).
    pub animations: Option<String>,
    /// Clip by state: `idle`, `walk`, `attack`, `hurt`, `death`, or the mod's
    /// own states (entity data `yog:state`).
    #[data(default)]
    pub clips: yog_data::Ordered<String>,
    /// Spawn egg colors `[base, spots]` (`0xRRGGBB`); none: no egg.
    pub spawn_egg: Option<[u32; 2]>,
    /// Mobs' AI goals, in priority order — the game's by name: `float`,
    /// `panic`, `melee_attack`, `wander`, `look_at_player`,
    /// `look_around`, `target_player`, `avoid_player`.
    #[data(default)]
    pub goals: Vec<String>,
    /// The mod handles each tick of every entity of this type
    /// (`yog_event::entity::game::Tick`, keyed by the type).
    #[data(default)]
    pub ticks: bool,
    /// A hanging thing's sound events when it's placed and when it breaks;
    /// default: a painting's.
    pub place_sound: Option<String>,
    pub break_sound: Option<String>,
}

fn eight() -> u32 {
    8
}

fn three() -> u32 {
    3
}

fn misc() -> String {
    "misc".into()
}

impl EntityDef {
    pub fn new(id: impl Into<String>, kind: EntityKind, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            name: None,
            kind,
            category: misc(),
            width,
            height,
            eye_height: None,
            fire_immune: false,
            no_gravity: false,
            pushable: None,
            solid: false,
            tracking_range: eight(),
            update_interval: three(),
            attributes: yog_data::Ordered::default(),
            model: None,
            animations: None,
            clips: yog_data::Ordered::default(),
            spawn_egg: None,
            goals: Vec::new(),
            ticks: false,
            place_sound: None,
            break_sound: None,
        }
    }
}
