//! The bus's services (`docs/design/bus.md`): `yog:<domain>/<name>`,
//! answered by whoever provides them — the host's actions, the runtime's
//! own services, mods'.

names! {
    service {
        /// What every service address starts with.
        PREFIX = crate::concatcp!(crate::ns::YOG, crate::sep::NAMESPACE),
    }

    /// `yog:block/…`: blocks.
    service_block {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "block", crate::sep::PATH),
        INSERT = crate::concatcp!(DOMAIN, "insert"),
        NBT = crate::concatcp!(DOMAIN, "nbt"),
        SET_NBT = crate::concatcp!(DOMAIN, "set_nbt"),
        STATE = crate::concatcp!(DOMAIN, "state"),
        SET_STATE = crate::concatcp!(DOMAIN, "set_state"),
        INFO = crate::concatcp!(DOMAIN, "info"),
        DESTROY = crate::concatcp!(DOMAIN, "destroy"),
        BONEMEAL = crate::concatcp!(DOMAIN, "bonemeal"),
        USE_ITEM = crate::concatcp!(DOMAIN, "use_item"),
        POUR_FLUID = crate::concatcp!(DOMAIN, "pour_fluid"),
        REDSTONE_POWER = crate::concatcp!(DOMAIN, "redstone_power"),
        REDSTONE_SIGNAL = crate::concatcp!(DOMAIN, "redstone_signal"),
        SET_REDSTONE_OUTPUT = crate::concatcp!(DOMAIN, "set_redstone_output"),
        INVENTORY_SLOT = crate::concatcp!(DOMAIN, "inventory_slot"),
        SET_INVENTORY_SLOT = crate::concatcp!(DOMAIN, "set_inventory_slot"),
    }

    /// `yog:item/…`: items and item stacks.
    service_item {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "item", crate::sep::PATH),
        TEXT = crate::concatcp!(DOMAIN, "text"),
        PICTURE = crate::concatcp!(DOMAIN, "picture"),
        SPAWN = crate::concatcp!(DOMAIN, "spawn"),
        ENTITY_STACK = crate::concatcp!(DOMAIN, "entity_stack"),
        SET_ENTITY_STACK = crate::concatcp!(DOMAIN, "set_entity_stack"),
        BLOCK = crate::concatcp!(DOMAIN, "block"),
    }

    /// `yog:world/…`: a world.
    service_world {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "world", crate::sep::PATH),
        GET_BLOCK = crate::concatcp!(DOMAIN, "get_block"),
        SET_BLOCK = crate::concatcp!(DOMAIN, "set_block"),
        TIME = crate::concatcp!(DOMAIN, "time"),
        SET_TIME = crate::concatcp!(DOMAIN, "set_time"),
        IS_RAINING = crate::concatcp!(DOMAIN, "is_raining"),
        SET_WEATHER = crate::concatcp!(DOMAIN, "set_weather"),
        IS_THUNDERING = crate::concatcp!(DOMAIN, "is_thundering"),
        SET_THUNDER = crate::concatcp!(DOMAIN, "set_thunder"),
        FILL_REGION = crate::concatcp!(DOMAIN, "fill_region"),
        SCAN_REGION = crate::concatcp!(DOMAIN, "scan_region"),
        PLACE_FEATURE = crate::concatcp!(DOMAIN, "place_feature"),
        EXPLODE = crate::concatcp!(DOMAIN, "explode"),
        DROP_LOOT = crate::concatcp!(DOMAIN, "drop_loot"),
        ENTITY_COUNT = crate::concatcp!(DOMAIN, "entity_count"),
        ENTITIES_IN_BOX = crate::concatcp!(DOMAIN, "entities_in_box"),
        RAYCAST_BLOCK = crate::concatcp!(DOMAIN, "raycast_block"),
        RAYCAST_ENTITY = crate::concatcp!(DOMAIN, "raycast_entity"),
    }

    /// `yog:chat/…`: messages to players.
    service_chat {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "chat", crate::sep::PATH),
        BROADCAST = crate::concatcp!(DOMAIN, "broadcast"),
        TELLRAW = crate::concatcp!(DOMAIN, "tellraw"),
        TITLE = crate::concatcp!(DOMAIN, "title"),
        ACTIONBAR = crate::concatcp!(DOMAIN, "actionbar"),
        ACTIONBAR_JSON = crate::concatcp!(DOMAIN, "actionbar_json"),
    }

    /// `yog:player/…`: players.
    service_player {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "player", crate::sep::PATH),
        GIVE_ITEM = crate::concatcp!(DOMAIN, "give_item"),
        TELEPORT = crate::concatcp!(DOMAIN, "teleport"),
        TELEPORT_TO_DIM = crate::concatcp!(DOMAIN, "teleport_to_dim"),
        KICK = crate::concatcp!(DOMAIN, "kick"),
        SET_GAMEMODE = crate::concatcp!(DOMAIN, "set_gamemode"),
        INVENTORY = crate::concatcp!(DOMAIN, "inventory"),
        SET_SLOT = crate::concatcp!(DOMAIN, "set_slot"),
        HELD_ITEM_NBT = crate::concatcp!(DOMAIN, "held_item_nbt"),
        SET_HELD_ITEM_NBT = crate::concatcp!(DOMAIN, "set_held_item_nbt"),
        OFFHAND_ITEM_NBT = crate::concatcp!(DOMAIN, "offhand_item_nbt"),
        SET_OFFHAND_ITEM_NBT = crate::concatcp!(DOMAIN, "set_offhand_item_nbt"),
        SLOT_ITEM = crate::concatcp!(DOMAIN, "slot_item"),
        SET_SLOT_ITEM = crate::concatcp!(DOMAIN, "set_slot_item"),
        ADD_XP = crate::concatcp!(DOMAIN, "add_xp"),
        SET_MAY_FLY = crate::concatcp!(DOMAIN, "set_may_fly"),
        INFO = crate::concatcp!(DOMAIN, "info"),
        SET_ITEM_COOLDOWN = crate::concatcp!(DOMAIN, "set_item_cooldown"),
        PLACE_BLOCK_FROM_SLOT = crate::concatcp!(DOMAIN, "place_block_from_slot"),
        ONLINE = crate::concatcp!(DOMAIN, "online"),
    }

    /// `yog:net/…`: the network.
    service_net {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "net", crate::sep::PATH),
        SEND_TO_PLAYER = crate::concatcp!(DOMAIN, "send_to_player"),
        SEND_TO_SERVER = crate::concatcp!(DOMAIN, "send_to_server"),
    }

    /// `yog:sound/…`: sounds.
    service_sound {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "sound", crate::sep::PATH),
        PLAY = crate::concatcp!(DOMAIN, "play"),
        PLAY_TO_PLAYER = crate::concatcp!(DOMAIN, "play_to_player"),
    }

    /// `yog:particle/…`: particles.
    service_particle {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "particle", crate::sep::PATH),
        SPAWN = crate::concatcp!(DOMAIN, "spawn"),
    }

    /// `yog:entity/…`: entities.
    service_entity {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "entity", crate::sep::PATH),
        TELEPORT = crate::concatcp!(DOMAIN, "teleport"),
        TELEPORT_TO_DIM = crate::concatcp!(DOMAIN, "teleport_to_dim"),
        POSITION = crate::concatcp!(DOMAIN, "position"),
        ROTATION = crate::concatcp!(DOMAIN, "rotation"),
        VELOCITY = crate::concatcp!(DOMAIN, "velocity"),
        SET_VELOCITY = crate::concatcp!(DOMAIN, "set_velocity"),
        ADD_VELOCITY = crate::concatcp!(DOMAIN, "add_velocity"),
        HEALTH = crate::concatcp!(DOMAIN, "health"),
        SET_HEALTH = crate::concatcp!(DOMAIN, "set_health"),
        KILL = crate::concatcp!(DOMAIN, "kill"),
        SPAWN = crate::concatcp!(DOMAIN, "spawn"),
        ADD_EFFECT = crate::concatcp!(DOMAIN, "add_effect"),
        REMOVE_EFFECT = crate::concatcp!(DOMAIN, "remove_effect"),
        CLEAR_EFFECTS = crate::concatcp!(DOMAIN, "clear_effects"),
        NBT = crate::concatcp!(DOMAIN, "nbt"),
        SET_NBT = crate::concatcp!(DOMAIN, "set_nbt"),
        ATTRIBUTE_BASE = crate::concatcp!(DOMAIN, "attribute_base"),
        SET_ATTRIBUTE_BASE = crate::concatcp!(DOMAIN, "set_attribute_base"),
        ATTRIBUTE_VALUE = crate::concatcp!(DOMAIN, "attribute_value"),
        INFO = crate::concatcp!(DOMAIN, "info"),
        DAMAGE = crate::concatcp!(DOMAIN, "damage"),
        SET_FIRE_TICKS = crate::concatcp!(DOMAIN, "set_fire_ticks"),
        VEHICLE = crate::concatcp!(DOMAIN, "vehicle"),
        PASSENGERS = crate::concatcp!(DOMAIN, "passengers"),
        MOUNT = crate::concatcp!(DOMAIN, "mount"),
        DISMOUNT = crate::concatcp!(DOMAIN, "dismount"),
        SET_FALL_DISTANCE = crate::concatcp!(DOMAIN, "set_fall_distance"),
        SET_DIMENSIONS = crate::concatcp!(DOMAIN, "set_dimensions"),
        DATA = crate::concatcp!(DOMAIN, "data"),
        SET_DATA = crate::concatcp!(DOMAIN, "set_data"),
    }

    /// `yog:tag/…`: tags.
    service_tag {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "tag", crate::sep::PATH),
        HAS_ITEM = crate::concatcp!(DOMAIN, "has_item"),
        HAS_BLOCK = crate::concatcp!(DOMAIN, "has_block"),
        HAS_ENTITY = crate::concatcp!(DOMAIN, "has_entity"),
        ITEMS = crate::concatcp!(DOMAIN, crate::word::ITEMS),
    }

    /// `yog:advancement/…`: advancements.
    service_advancement {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "advancement", crate::sep::PATH),
        GRANT = crate::concatcp!(DOMAIN, "grant"),
        DONE = crate::concatcp!(DOMAIN, "done"),
    }

    /// `yog:scoreboard/…`: the scoreboard.
    service_scoreboard {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "scoreboard", crate::sep::PATH),
        GET = crate::concatcp!(DOMAIN, "get"),
        SET = crate::concatcp!(DOMAIN, "set"),
        ADD = crate::concatcp!(DOMAIN, "add"),
    }

    /// `yog:bossbar/…`: boss bars.
    service_bossbar {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "bossbar", crate::sep::PATH),
        CREATE = crate::concatcp!(DOMAIN, "create"),
        REMOVE = crate::concatcp!(DOMAIN, "remove"),
        SET_TITLE = crate::concatcp!(DOMAIN, "set_title"),
        SET_PROGRESS = crate::concatcp!(DOMAIN, "set_progress"),
        SET_COLOR = crate::concatcp!(DOMAIN, "set_color"),
        ADD_PLAYER = crate::concatcp!(DOMAIN, "add_player"),
        REMOVE_PLAYER = crate::concatcp!(DOMAIN, "remove_player"),
        SET_VISIBLE = crate::concatcp!(DOMAIN, "set_visible"),
    }

    /// `yog:game/…`: the game.
    service_game {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "game", crate::sep::PATH),
        DIR = crate::concatcp!(DOMAIN, "dir"),
        WORLD_DIR = crate::concatcp!(DOMAIN, "world_dir"),
        TRANSLATE = crate::concatcp!(DOMAIN, "translate"),
        MODS = crate::concatcp!(DOMAIN, "mods"),
        MOD_LOADED = crate::concatcp!(DOMAIN, "mod_loaded"),
        PLATFORM_MODS = crate::concatcp!(DOMAIN, "platform_mods"),
    }

    /// Which list an installed mod is on.
    mod_source {
        /// Yog's own mods.
        YOG = crate::ns::YOG,
        /// The loader's (Java) mods.
        PLATFORM = "platform",
    }

    /// `yog:client/…`: this client's own actions (nothing on a server).
    service_client {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "client", crate::sep::PATH),
        PLAY_SOUND = crate::concatcp!(DOMAIN, "play_sound"),
        STOP_SOUND = crate::concatcp!(DOMAIN, "stop_sound"),
        HELD_ITEM = crate::concatcp!(DOMAIN, "held_item"),
        SET_BLOCK_TINT = crate::concatcp!(DOMAIN, "set_block_tint"),
        KEY_DOWN = crate::concatcp!(DOMAIN, "key_down"),
        ATTRIBUTE_VALUE = crate::concatcp!(DOMAIN, "attribute_value"),
        ADVANCEMENT_DONE = crate::concatcp!(DOMAIN, "advancement_done"),
        BODY_TRACK = crate::concatcp!(DOMAIN, "body_track"),
        BODY_TRACK_TYPE = crate::concatcp!(DOMAIN, "body_track_type"),
        SET_DIMENSIONS = crate::concatcp!(DOMAIN, "set_dimensions"),
        ENTITY_DATA = crate::concatcp!(DOMAIN, "entity_data"),
        SCREEN_SLOTS = crate::concatcp!(DOMAIN, "screen_slots"),
    }

    /// `yog:client/local/…`: what this client's player is and sees.
    service_client_local {
        DOMAIN = crate::concatcp!(crate::service_client::DOMAIN, "local", crate::sep::PATH),
        STATE = crate::concatcp!(DOMAIN, "state"),
        INVENTORY = crate::concatcp!(DOMAIN, "inventory"),
        SCAN = crate::concatcp!(DOMAIN, "scan"),
        AREA = crate::concatcp!(DOMAIN, "area"),
        ENTITIES = crate::concatcp!(DOMAIN, "entities"),
    }

    /// `yog:client/control/…`: this client's player doing what a player
    /// does — its own keys and clicks, for automation.
    service_client_control {
        DOMAIN = crate::concatcp!(crate::service_client::DOMAIN, "control", crate::sep::PATH),
        DRIVE = crate::concatcp!(DOMAIN, "drive"),
        RELEASE = crate::concatcp!(DOMAIN, "release"),
        LOOK = crate::concatcp!(DOMAIN, "look"),
        ATTACK = crate::concatcp!(DOMAIN, "attack"),
        USE_ITEM = crate::concatcp!(DOMAIN, "use_item"),
        MINE = crate::concatcp!(DOMAIN, "mine"),
        STOP_MINING = crate::concatcp!(DOMAIN, "stop_mining"),
        PLACE = crate::concatcp!(DOMAIN, "place"),
        SELECT_SLOT = crate::concatcp!(DOMAIN, "select_slot"),
        CLICK = crate::concatcp!(DOMAIN, "click"),
        CHAT = crate::concatcp!(DOMAIN, "chat"),
    }

    /// `yog:ui/…`: mods' screens.
    service_ui {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "ui", crate::sep::PATH),
        OPEN = crate::concatcp!(DOMAIN, "open"),
        OPEN_WITH_FLAGS = crate::concatcp!(DOMAIN, "open_with_flags"),
        CLOSE = crate::concatcp!(DOMAIN, "close"),
    }

    /// `yog:bus/…`: what the bus carries, described (provided by the runtime).
    service_bus {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "bus", crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, "declare"),
        DESCRIBE = crate::concatcp!(DOMAIN, "describe"),
        LIST = crate::concatcp!(DOMAIN, "list"),
        FIND = crate::concatcp!(DOMAIN, "find"),
    }

    /// `yog:content/…`: what mods register (provided by the runtime).
    service_content {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "content", crate::sep::PATH),
        REGISTER = crate::concatcp!(DOMAIN, "register"),
    }

    /// `yog:jvm/…`: Java mods' code, reached by reflection.
    service_jvm {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "jvm", crate::sep::PATH),
        CALL = crate::concatcp!(DOMAIN, "call"),
    }

    /// `yog:menu/…`: the menu entries mods declare (provided by the runtime).
    service_menu {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "menu", crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, "declare"),
        DECLARED = crate::concatcp!(DOMAIN, "declared"),
    }

    /// `yog:key_binding/…`: the key bindings mods declare (provided by the runtime).
    service_key_binding {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "key_binding", crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, "declare"),
        DECLARED = crate::concatcp!(DOMAIN, "declared"),
    }

    /// `yog:command/…`: the commands mods declare (provided by the runtime).
    service_command {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "command", crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, "declare"),
        DECLARED = crate::concatcp!(DOMAIN, "declared"),
    }
}
