//! The bus's services (`docs/design/bus.md`): `yog:<domain>/<name>`,
//! answered by whoever provides them — the host's actions, the runtime's
//! own services, mods'.

names! {
    service {
        /// What every service address starts with.
        PREFIX = concatcp!(ns::YOG, sep::NAMESPACE),
    }

    /// `yog:block/…`: blocks.
    service_block {
        DOMAIN = concatcp!(service::PREFIX, "block", sep::PATH),
        INSERT = concatcp!(DOMAIN, "insert"),
        NBT = concatcp!(DOMAIN, "nbt"),
        SET_NBT = concatcp!(DOMAIN, "set_nbt"),
        STATE = concatcp!(DOMAIN, "state"),
        SET_STATE = concatcp!(DOMAIN, "set_state"),
        INFO = concatcp!(DOMAIN, "info"),
        DESTROY = concatcp!(DOMAIN, "destroy"),
        BONEMEAL = concatcp!(DOMAIN, "bonemeal"),
        USE_ITEM = concatcp!(DOMAIN, "use_item"),
        POUR_FLUID = concatcp!(DOMAIN, "pour_fluid"),
        REDSTONE_POWER = concatcp!(DOMAIN, "redstone_power"),
        REDSTONE_SIGNAL = concatcp!(DOMAIN, "redstone_signal"),
        SET_REDSTONE_OUTPUT = concatcp!(DOMAIN, "set_redstone_output"),
        INVENTORY_SLOT = concatcp!(DOMAIN, "inventory_slot"),
        SET_INVENTORY_SLOT = concatcp!(DOMAIN, "set_inventory_slot"),
    }

    /// `yog:item/…`: items and item stacks.
    service_item {
        DOMAIN = concatcp!(service::PREFIX, "item", sep::PATH),
        TEXT = concatcp!(DOMAIN, "text"),
        PICTURE = concatcp!(DOMAIN, "picture"),
        SPAWN = concatcp!(DOMAIN, "spawn"),
        ENTITY_STACK = concatcp!(DOMAIN, "entity_stack"),
        SET_ENTITY_STACK = concatcp!(DOMAIN, "set_entity_stack"),
        BLOCK = concatcp!(DOMAIN, "block"),
    }

    /// `yog:world/…`: a world.
    service_world {
        DOMAIN = concatcp!(service::PREFIX, "world", sep::PATH),
        GET_BLOCK = concatcp!(DOMAIN, "get_block"),
        SET_BLOCK = concatcp!(DOMAIN, "set_block"),
        TIME = concatcp!(DOMAIN, "time"),
        SET_TIME = concatcp!(DOMAIN, "set_time"),
        IS_RAINING = concatcp!(DOMAIN, "is_raining"),
        SET_WEATHER = concatcp!(DOMAIN, "set_weather"),
        IS_THUNDERING = concatcp!(DOMAIN, "is_thundering"),
        SET_THUNDER = concatcp!(DOMAIN, "set_thunder"),
        FILL_REGION = concatcp!(DOMAIN, "fill_region"),
        SCAN_REGION = concatcp!(DOMAIN, "scan_region"),
        PLACE_FEATURE = concatcp!(DOMAIN, "place_feature"),
        EXPLODE = concatcp!(DOMAIN, "explode"),
        DROP_LOOT = concatcp!(DOMAIN, "drop_loot"),
        ENTITY_COUNT = concatcp!(DOMAIN, "entity_count"),
        ENTITIES_IN_BOX = concatcp!(DOMAIN, "entities_in_box"),
        RAYCAST_BLOCK = concatcp!(DOMAIN, "raycast_block"),
        RAYCAST_ENTITY = concatcp!(DOMAIN, "raycast_entity"),
    }

    /// `yog:chat/…`: messages to players.
    service_chat {
        DOMAIN = concatcp!(service::PREFIX, "chat", sep::PATH),
        BROADCAST = concatcp!(DOMAIN, "broadcast"),
        TELLRAW = concatcp!(DOMAIN, "tellraw"),
        TITLE = concatcp!(DOMAIN, "title"),
        ACTIONBAR = concatcp!(DOMAIN, "actionbar"),
        ACTIONBAR_JSON = concatcp!(DOMAIN, "actionbar_json"),
    }

    /// `yog:player/…`: players.
    service_player {
        DOMAIN = concatcp!(service::PREFIX, "player", sep::PATH),
        GIVE_ITEM = concatcp!(DOMAIN, "give_item"),
        TELEPORT = concatcp!(DOMAIN, "teleport"),
        TELEPORT_TO_DIM = concatcp!(DOMAIN, "teleport_to_dim"),
        KICK = concatcp!(DOMAIN, "kick"),
        SET_GAMEMODE = concatcp!(DOMAIN, "set_gamemode"),
        INVENTORY = concatcp!(DOMAIN, "inventory"),
        SET_SLOT = concatcp!(DOMAIN, "set_slot"),
        HELD_ITEM_NBT = concatcp!(DOMAIN, "held_item_nbt"),
        SET_HELD_ITEM_NBT = concatcp!(DOMAIN, "set_held_item_nbt"),
        OFFHAND_ITEM_NBT = concatcp!(DOMAIN, "offhand_item_nbt"),
        SET_OFFHAND_ITEM_NBT = concatcp!(DOMAIN, "set_offhand_item_nbt"),
        SLOT_ITEM = concatcp!(DOMAIN, "slot_item"),
        SET_SLOT_ITEM = concatcp!(DOMAIN, "set_slot_item"),
        ADD_XP = concatcp!(DOMAIN, "add_xp"),
        SET_MAY_FLY = concatcp!(DOMAIN, "set_may_fly"),
        INFO = concatcp!(DOMAIN, "info"),
        SET_ITEM_COOLDOWN = concatcp!(DOMAIN, "set_item_cooldown"),
        PLACE_BLOCK_FROM_SLOT = concatcp!(DOMAIN, "place_block_from_slot"),
        ONLINE = concatcp!(DOMAIN, "online"),
    }

    /// `yog:net/…`: the network.
    service_net {
        DOMAIN = concatcp!(service::PREFIX, "net", sep::PATH),
        SEND_TO_PLAYER = concatcp!(DOMAIN, "send_to_player"),
        SEND_TO_SERVER = concatcp!(DOMAIN, "send_to_server"),
    }

    /// `yog:sound/…`: sounds.
    service_sound {
        DOMAIN = concatcp!(service::PREFIX, "sound", sep::PATH),
        PLAY = concatcp!(DOMAIN, "play"),
        PLAY_TO_PLAYER = concatcp!(DOMAIN, "play_to_player"),
    }

    /// `yog:particle/…`: particles.
    service_particle {
        DOMAIN = concatcp!(service::PREFIX, "particle", sep::PATH),
        SPAWN = concatcp!(DOMAIN, "spawn"),
    }

    /// `yog:entity/…`: entities.
    service_entity {
        DOMAIN = concatcp!(service::PREFIX, "entity", sep::PATH),
        TELEPORT = concatcp!(DOMAIN, "teleport"),
        TELEPORT_TO_DIM = concatcp!(DOMAIN, "teleport_to_dim"),
        POSITION = concatcp!(DOMAIN, "position"),
        ROTATION = concatcp!(DOMAIN, "rotation"),
        VELOCITY = concatcp!(DOMAIN, "velocity"),
        SET_VELOCITY = concatcp!(DOMAIN, "set_velocity"),
        ADD_VELOCITY = concatcp!(DOMAIN, "add_velocity"),
        HEALTH = concatcp!(DOMAIN, "health"),
        SET_HEALTH = concatcp!(DOMAIN, "set_health"),
        KILL = concatcp!(DOMAIN, "kill"),
        SPAWN = concatcp!(DOMAIN, "spawn"),
        ADD_EFFECT = concatcp!(DOMAIN, "add_effect"),
        REMOVE_EFFECT = concatcp!(DOMAIN, "remove_effect"),
        CLEAR_EFFECTS = concatcp!(DOMAIN, "clear_effects"),
        NBT = concatcp!(DOMAIN, "nbt"),
        SET_NBT = concatcp!(DOMAIN, "set_nbt"),
        ATTRIBUTE_BASE = concatcp!(DOMAIN, "attribute_base"),
        SET_ATTRIBUTE_BASE = concatcp!(DOMAIN, "set_attribute_base"),
        ATTRIBUTE_VALUE = concatcp!(DOMAIN, "attribute_value"),
        INFO = concatcp!(DOMAIN, "info"),
        DAMAGE = concatcp!(DOMAIN, "damage"),
        SET_FIRE_TICKS = concatcp!(DOMAIN, "set_fire_ticks"),
        VEHICLE = concatcp!(DOMAIN, "vehicle"),
        PASSENGERS = concatcp!(DOMAIN, "passengers"),
        MOUNT = concatcp!(DOMAIN, "mount"),
        DISMOUNT = concatcp!(DOMAIN, "dismount"),
        SET_FALL_DISTANCE = concatcp!(DOMAIN, "set_fall_distance"),
        SET_DIMENSIONS = concatcp!(DOMAIN, "set_dimensions"),
        DATA = concatcp!(DOMAIN, "data"),
        SET_DATA = concatcp!(DOMAIN, "set_data"),
    }

    /// `yog:tag/…`: tags.
    service_tag {
        DOMAIN = concatcp!(service::PREFIX, "tag", sep::PATH),
        HAS_ITEM = concatcp!(DOMAIN, "has_item"),
        HAS_BLOCK = concatcp!(DOMAIN, "has_block"),
        HAS_ENTITY = concatcp!(DOMAIN, "has_entity"),
    }

    /// `yog:advancement/…`: advancements.
    service_advancement {
        DOMAIN = concatcp!(service::PREFIX, "advancement", sep::PATH),
        GRANT = concatcp!(DOMAIN, "grant"),
        DONE = concatcp!(DOMAIN, "done"),
    }

    /// `yog:scoreboard/…`: the scoreboard.
    service_scoreboard {
        DOMAIN = concatcp!(service::PREFIX, "scoreboard", sep::PATH),
        GET = concatcp!(DOMAIN, "get"),
        SET = concatcp!(DOMAIN, "set"),
        ADD = concatcp!(DOMAIN, "add"),
    }

    /// `yog:bossbar/…`: boss bars.
    service_bossbar {
        DOMAIN = concatcp!(service::PREFIX, "bossbar", sep::PATH),
        CREATE = concatcp!(DOMAIN, "create"),
        REMOVE = concatcp!(DOMAIN, "remove"),
        SET_TITLE = concatcp!(DOMAIN, "set_title"),
        SET_PROGRESS = concatcp!(DOMAIN, "set_progress"),
        SET_COLOR = concatcp!(DOMAIN, "set_color"),
        ADD_PLAYER = concatcp!(DOMAIN, "add_player"),
        REMOVE_PLAYER = concatcp!(DOMAIN, "remove_player"),
        SET_VISIBLE = concatcp!(DOMAIN, "set_visible"),
    }

    /// `yog:game/…`: the game.
    service_game {
        DOMAIN = concatcp!(service::PREFIX, "game", sep::PATH),
        DIR = concatcp!(DOMAIN, "dir"),
        WORLD_DIR = concatcp!(DOMAIN, "world_dir"),
        TRANSLATE = concatcp!(DOMAIN, "translate"),
        MODS = concatcp!(DOMAIN, "mods"),
        MOD_LOADED = concatcp!(DOMAIN, "mod_loaded"),
    }

    /// `yog:client/…`: this client's own actions (nothing on a server).
    service_client {
        DOMAIN = concatcp!(service::PREFIX, "client", sep::PATH),
        PLAY_SOUND = concatcp!(DOMAIN, "play_sound"),
        STOP_SOUND = concatcp!(DOMAIN, "stop_sound"),
        HELD_ITEM = concatcp!(DOMAIN, "held_item"),
        SET_BLOCK_TINT = concatcp!(DOMAIN, "set_block_tint"),
        KEY_DOWN = concatcp!(DOMAIN, "key_down"),
        ATTRIBUTE_VALUE = concatcp!(DOMAIN, "attribute_value"),
        ADVANCEMENT_DONE = concatcp!(DOMAIN, "advancement_done"),
        OP = concatcp!(DOMAIN, "op"),
        BODY_TRACK = concatcp!(DOMAIN, "body_track"),
        BODY_TRACK_TYPE = concatcp!(DOMAIN, "body_track_type"),
        SET_DIMENSIONS = concatcp!(DOMAIN, "set_dimensions"),
        ENTITY_DATA = concatcp!(DOMAIN, "entity_data"),
    }

    /// `yog:ui/…`: mods' screens.
    service_ui {
        DOMAIN = concatcp!(service::PREFIX, "ui", sep::PATH),
        OPEN = concatcp!(DOMAIN, "open"),
        OPEN_WITH_FLAGS = concatcp!(DOMAIN, "open_with_flags"),
        CLOSE = concatcp!(DOMAIN, "close"),
    }

    /// `yog:menu/…`: the menu entries mods declare (provided by the runtime).
    service_menu {
        DOMAIN = concatcp!(service::PREFIX, "menu", sep::PATH),
        DECLARE = concatcp!(DOMAIN, "declare"),
        DECLARED = concatcp!(DOMAIN, "declared"),
    }

    /// `yog:key_binding/…`: the key bindings mods declare (provided by the runtime).
    service_key_binding {
        DOMAIN = concatcp!(service::PREFIX, "key_binding", sep::PATH),
        DECLARE = concatcp!(DOMAIN, "declare"),
        DECLARED = concatcp!(DOMAIN, "declared"),
    }

    /// `yog:command/…`: the commands mods declare (provided by the runtime).
    service_command {
        DOMAIN = concatcp!(service::PREFIX, "command", sep::PATH),
        DECLARE = concatcp!(DOMAIN, "declare"),
        DECLARED = concatcp!(DOMAIN, "declared"),
    }
}
