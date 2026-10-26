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
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::BLOCK, crate::sep::PATH),
        INSERT = crate::concatcp!(DOMAIN, "insert"),
        NBT = crate::concatcp!(DOMAIN, crate::bus_word::NBT),
        SET_NBT = crate::concatcp!(DOMAIN, crate::bus_word::SET_NBT),
        STATE = crate::concatcp!(DOMAIN, crate::word::STATE),
        SET_STATE = crate::concatcp!(DOMAIN, "set_state"),
        INFO = crate::concatcp!(DOMAIN, crate::bus_word::INFO),
        DESTROY = crate::concatcp!(DOMAIN, "destroy"),
        BONEMEAL = crate::concatcp!(DOMAIN, "bonemeal"),
        USE_ITEM = crate::concatcp!(DOMAIN, crate::bus_word::USE_ITEM),
        POUR_FLUID = crate::concatcp!(DOMAIN, "pour_fluid"),
        REDSTONE_POWER = crate::concatcp!(DOMAIN, "redstone_power"),
        REDSTONE_SIGNAL = crate::concatcp!(DOMAIN, "redstone_signal"),
        SET_REDSTONE_OUTPUT = crate::concatcp!(DOMAIN, "set_redstone_output"),
        INVENTORY_SLOT = crate::concatcp!(DOMAIN, "inventory_slot"),
        SET_INVENTORY_SLOT = crate::concatcp!(DOMAIN, "set_inventory_slot"),
    }

    /// `yog:item/…`: items and item stacks.
    service_item {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::ITEM, crate::sep::PATH),
        TEXT = crate::concatcp!(DOMAIN, crate::word::TEXT),
        PICTURE = crate::concatcp!(DOMAIN, "picture"),
        SPAWN = crate::concatcp!(DOMAIN, crate::bus_word::SPAWN),
        ENTITY_STACK = crate::concatcp!(DOMAIN, "entity_stack"),
        SET_ENTITY_STACK = crate::concatcp!(DOMAIN, "set_entity_stack"),
        BLOCK = crate::concatcp!(DOMAIN, crate::word::BLOCK),
        FOOD = crate::concatcp!(DOMAIN, "food"),
    }

    /// `yog:world/…`: a world.
    service_world {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::bus_word::WORLD, crate::sep::PATH),
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
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::bus_word::CHAT, crate::sep::PATH),
        BROADCAST = crate::concatcp!(DOMAIN, "broadcast"),
        TELLRAW = crate::concatcp!(DOMAIN, "tellraw"),
        TITLE = crate::concatcp!(DOMAIN, "title"),
        ACTIONBAR = crate::concatcp!(DOMAIN, "actionbar"),
        ACTIONBAR_JSON = crate::concatcp!(DOMAIN, "actionbar_json"),
    }

    /// `yog:player/…`: players.
    service_player {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::PLAYER, crate::sep::PATH),
        GIVE_ITEM = crate::concatcp!(DOMAIN, "give_item"),
        TELEPORT = crate::concatcp!(DOMAIN, crate::bus_word::TELEPORT),
        TELEPORT_TO_DIM = crate::concatcp!(DOMAIN, crate::bus_word::TELEPORT_TO_DIM),
        KICK = crate::concatcp!(DOMAIN, "kick"),
        SET_GAMEMODE = crate::concatcp!(DOMAIN, "set_gamemode"),
        INVENTORY = crate::concatcp!(DOMAIN, crate::kind::INVENTORY),
        SET_SLOT = crate::concatcp!(DOMAIN, "set_slot"),
        HELD_ITEM_NBT = crate::concatcp!(DOMAIN, "held_item_nbt"),
        SET_HELD_ITEM_NBT = crate::concatcp!(DOMAIN, "set_held_item_nbt"),
        OFFHAND_ITEM_NBT = crate::concatcp!(DOMAIN, "offhand_item_nbt"),
        SET_OFFHAND_ITEM_NBT = crate::concatcp!(DOMAIN, "set_offhand_item_nbt"),
        SLOT_ITEM = crate::concatcp!(DOMAIN, "slot_item"),
        SET_SLOT_ITEM = crate::concatcp!(DOMAIN, "set_slot_item"),
        ADD_XP = crate::concatcp!(DOMAIN, "add_xp"),
        SET_MAY_FLY = crate::concatcp!(DOMAIN, "set_may_fly"),
        INFO = crate::concatcp!(DOMAIN, crate::bus_word::INFO),
        SET_ITEM_COOLDOWN = crate::concatcp!(DOMAIN, "set_item_cooldown"),
        PLACE_BLOCK_FROM_SLOT = crate::concatcp!(DOMAIN, "place_block_from_slot"),
        ONLINE = crate::concatcp!(DOMAIN, "online"),
    }

    /// `yog:net/…`: the network.
    /// `yog:media/…`: pictures and sounds as the game takes them.
    service_media {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "media", crate::sep::PATH),
        /// A `.yog`'s yimage as the PNG the game reads (the runtime answers).
        PNG = crate::concatcp!(DOMAIN, crate::word::PNG),
    }

    /// `yog:video/…`: the videos a client plays (the runtime answers).
    service_video {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::VIDEO, crate::sep::PATH),
        OPEN = crate::concatcp!(DOMAIN, crate::bus_word::OPEN),
        PLAY = crate::concatcp!(DOMAIN, crate::bus_word::PLAY),
        SEEK = crate::concatcp!(DOMAIN, "seek"),
        CLOSE = crate::concatcp!(DOMAIN, crate::bus_word::CLOSE),
        /// Its picture now, to draw.
        FRAME = crate::concatcp!(DOMAIN, crate::word::FRAME),
        /// A video full screen over the game.
        CUTSCENE = crate::concatcp!(DOMAIN, crate::word::CUTSCENE),
    }

    service_net {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::bus_word::NET, crate::sep::PATH),
        SEND_TO_PLAYER = crate::concatcp!(DOMAIN, "send_to_player"),
        SEND_TO_SERVER = crate::concatcp!(DOMAIN, "send_to_server"),
    }

    /// `yog:sound/…`: sounds.
    service_sound {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::SOUND, crate::sep::PATH),
        PLAY = crate::concatcp!(DOMAIN, crate::bus_word::PLAY),
        PLAY_TO_PLAYER = crate::concatcp!(DOMAIN, "play_to_player"),
    }

    /// `yog:particle/…`: particles.
    service_particle {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::model::PARTICLE, crate::sep::PATH),
        SPAWN = crate::concatcp!(DOMAIN, crate::bus_word::SPAWN),
    }

    /// `yog:entity/…`: entities.
    service_entity {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::kind::ENTITY, crate::sep::PATH),
        TELEPORT = crate::concatcp!(DOMAIN, crate::bus_word::TELEPORT),
        TELEPORT_TO_DIM = crate::concatcp!(DOMAIN, crate::bus_word::TELEPORT_TO_DIM),
        POSITION = crate::concatcp!(DOMAIN, "position"),
        ROTATION = crate::concatcp!(DOMAIN, crate::model::ROTATION),
        VELOCITY = crate::concatcp!(DOMAIN, crate::mirror_word::VELOCITY),
        SET_VELOCITY = crate::concatcp!(DOMAIN, "set_velocity"),
        ADD_VELOCITY = crate::concatcp!(DOMAIN, "add_velocity"),
        HEALTH = crate::concatcp!(DOMAIN, crate::mirror_word::HEALTH),
        SET_HEALTH = crate::concatcp!(DOMAIN, "set_health"),
        KILL = crate::concatcp!(DOMAIN, "kill"),
        SPAWN = crate::concatcp!(DOMAIN, crate::bus_word::SPAWN),
        ADD_EFFECT = crate::concatcp!(DOMAIN, "add_effect"),
        REMOVE_EFFECT = crate::concatcp!(DOMAIN, "remove_effect"),
        CLEAR_EFFECTS = crate::concatcp!(DOMAIN, "clear_effects"),
        NBT = crate::concatcp!(DOMAIN, crate::bus_word::NBT),
        SET_NBT = crate::concatcp!(DOMAIN, crate::bus_word::SET_NBT),
        ATTRIBUTE_BASE = crate::concatcp!(DOMAIN, "attribute_base"),
        SET_ATTRIBUTE_BASE = crate::concatcp!(DOMAIN, "set_attribute_base"),
        ATTRIBUTE_VALUE = crate::concatcp!(DOMAIN, crate::bus_word::ATTRIBUTE_VALUE),
        INFO = crate::concatcp!(DOMAIN, crate::bus_word::INFO),
        DAMAGE = crate::concatcp!(DOMAIN, crate::word::DAMAGE),
        SET_FIRE_TICKS = crate::concatcp!(DOMAIN, "set_fire_ticks"),
        VEHICLE = crate::concatcp!(DOMAIN, "vehicle"),
        PASSENGERS = crate::concatcp!(DOMAIN, "passengers"),
        MOUNT = crate::concatcp!(DOMAIN, "mount"),
        DISMOUNT = crate::concatcp!(DOMAIN, "dismount"),
        SET_FALL_DISTANCE = crate::concatcp!(DOMAIN, "set_fall_distance"),
        SET_DIMENSIONS = crate::concatcp!(DOMAIN, crate::bus_word::SET_DIMENSIONS),
        DATA = crate::concatcp!(DOMAIN, crate::word::DATA),
        SET_DATA = crate::concatcp!(DOMAIN, "set_data"),
    }

    /// `yog:tag/…`: tags.
    service_tag {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::TAG, crate::sep::PATH),
        HAS_ITEM = crate::concatcp!(DOMAIN, "has_item"),
        HAS_BLOCK = crate::concatcp!(DOMAIN, "has_block"),
        HAS_ENTITY = crate::concatcp!(DOMAIN, "has_entity"),
        ITEMS = crate::concatcp!(DOMAIN, crate::word::ITEMS),
    }

    /// `yog:advancement/…`: advancements.
    service_advancement {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::kind::ADVANCEMENT, crate::sep::PATH),
        GRANT = crate::concatcp!(DOMAIN, "grant"),
        DONE = crate::concatcp!(DOMAIN, "done"),
    }

    /// `yog:scoreboard/…`: the scoreboard.
    service_scoreboard {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "scoreboard", crate::sep::PATH),
        GET = crate::concatcp!(DOMAIN, crate::bus_word::GET),
        SET = crate::concatcp!(DOMAIN, crate::serve_param::SET),
        ADD = crate::concatcp!(DOMAIN, "add"),
    }

    /// `yog:bossbar/…`: boss bars.
    service_bossbar {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "bossbar", crate::sep::PATH),
        CREATE = crate::concatcp!(DOMAIN, "create"),
        REMOVE = crate::concatcp!(DOMAIN, crate::word::REMOVE),
        SET_TITLE = crate::concatcp!(DOMAIN, "set_title"),
        SET_PROGRESS = crate::concatcp!(DOMAIN, "set_progress"),
        SET_COLOR = crate::concatcp!(DOMAIN, "set_color"),
        ADD_PLAYER = crate::concatcp!(DOMAIN, "add_player"),
        REMOVE_PLAYER = crate::concatcp!(DOMAIN, "remove_player"),
        SET_VISIBLE = crate::concatcp!(DOMAIN, "set_visible"),
    }

    /// `yog:game/…`: the game.
    service_game {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::bus_word::GAME, crate::sep::PATH),
        DIR = crate::concatcp!(DOMAIN, "dir"),
        WORLD_DIR = crate::concatcp!(DOMAIN, "world_dir"),
        TRANSLATE = crate::concatcp!(DOMAIN, "translate"),
        MODS = crate::concatcp!(DOMAIN, crate::word::MODS),
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
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::bus_word::CLIENT, crate::sep::PATH),
        PLAY_SOUND = crate::concatcp!(DOMAIN, "play_sound"),
        STOP_SOUND = crate::concatcp!(DOMAIN, "stop_sound"),
        HELD_ITEM = crate::concatcp!(DOMAIN, "held_item"),
        SET_BLOCK_TINT = crate::concatcp!(DOMAIN, "set_block_tint"),
        ONLY_BLOCKS = crate::concatcp!(DOMAIN, "only_blocks"),
        KEY_DOWN = crate::concatcp!(DOMAIN, "key_down"),
        KEY_NAME = crate::concatcp!(DOMAIN, "key_name"),
        ATTRIBUTE_VALUE = crate::concatcp!(DOMAIN, crate::bus_word::ATTRIBUTE_VALUE),
        ADVANCEMENT_DONE = crate::concatcp!(DOMAIN, "advancement_done"),
        BODY_TRACK = crate::concatcp!(DOMAIN, "body_track"),
        BODY_TRACK_TYPE = crate::concatcp!(DOMAIN, "body_track_type"),
        SET_DIMENSIONS = crate::concatcp!(DOMAIN, crate::bus_word::SET_DIMENSIONS),
        ENTITY_DATA = crate::concatcp!(DOMAIN, "entity_data"),
        SCREEN_SLOTS = crate::concatcp!(DOMAIN, "screen_slots"),
    }

    /// `yog:client/local/…`: what this client's player is and sees.
    service_client_local {
        DOMAIN = crate::concatcp!(crate::service_client::DOMAIN, "local", crate::sep::PATH),
        STATE = crate::concatcp!(DOMAIN, crate::word::STATE),
        INVENTORY = crate::concatcp!(DOMAIN, crate::kind::INVENTORY),
        SCAN = crate::concatcp!(DOMAIN, "scan"),
        AREA = crate::concatcp!(DOMAIN, "area"),
        ENTITIES = crate::concatcp!(DOMAIN, crate::word::ENTITIES),
        DIG_SPEEDS = crate::concatcp!(DOMAIN, "dig_speeds"),
        RAYCAST = crate::concatcp!(DOMAIN, "raycast"),
        TARGET = crate::concatcp!(DOMAIN, crate::word::TARGET),
        MESSAGE = crate::concatcp!(DOMAIN, crate::word::MESSAGE),
    }

    /// `yog:client/control/…`: this client's player doing what a player
    /// does — its own keys and clicks, for automation.
    service_client_control {
        DOMAIN = crate::concatcp!(crate::service_client::DOMAIN, "control", crate::sep::PATH),
        DRIVE = crate::concatcp!(DOMAIN, "drive"),
        RELEASE = crate::concatcp!(DOMAIN, crate::bus_word::RELEASE),
        LOOK = crate::concatcp!(DOMAIN, "look"),
        ATTACK = crate::concatcp!(DOMAIN, crate::bus_word::ATTACK),
        USE_ITEM = crate::concatcp!(DOMAIN, crate::bus_word::USE_ITEM),
        MINE = crate::concatcp!(DOMAIN, "mine"),
        STOP_MINING = crate::concatcp!(DOMAIN, "stop_mining"),
        PLACE = crate::concatcp!(DOMAIN, crate::bus_word::PLACE),
        SELECT_SLOT = crate::concatcp!(DOMAIN, "select_slot"),
        CLICK = crate::concatcp!(DOMAIN, "click"),
        CHAT = crate::concatcp!(DOMAIN, crate::bus_word::CHAT),
    }

    /// `yog:ui/…`: mods' screens.
    service_ui {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::UI, crate::sep::PATH),
        OPEN = crate::concatcp!(DOMAIN, crate::bus_word::OPEN),
        OPEN_WITH_FLAGS = crate::concatcp!(DOMAIN, "open_with_flags"),
        CLOSE = crate::concatcp!(DOMAIN, crate::bus_word::CLOSE),
    }

    /// `yog:bus/…`: what the bus carries, described (provided by the runtime).
    service_bus {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "bus", crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, crate::bus_word::DECLARE),
        DESCRIBE = crate::concatcp!(DOMAIN, "describe"),
        LIST = crate::concatcp!(DOMAIN, crate::shape_kind::LIST),
        FIND = crate::concatcp!(DOMAIN, "find"),
    }

    /// `yog:storage/…/<mod>`: a mod's declared state in its stores (each
    /// mod's native provides its own, keyed by its id).
    service_storage {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::STORAGE, crate::sep::PATH),
        READ = crate::concatcp!(DOMAIN, "read"),
        WRITE = crate::concatcp!(DOMAIN, crate::word::WRITE),
        REMOVE = crate::concatcp!(DOMAIN, crate::word::REMOVE),
        KEYS = crate::concatcp!(DOMAIN, crate::mirror_word::KEYS),
    }

    /// `yog:content/…`: what mods register (provided by the runtime).
    service_content {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::word::CONTENT, crate::sep::PATH),
        REGISTER = crate::concatcp!(DOMAIN, "register"),
    }

    /// `yog:jvm/…`: Java mods' code, reached by reflection.
    service_jvm {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "jvm", crate::sep::PATH),
        CALL = crate::concatcp!(DOMAIN, crate::bus_word::CALL),
    }

    /// `yog:menu/…`: the menu entries mods declare (provided by the runtime).
    service_menu {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "menu", crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, crate::bus_word::DECLARE),
        DECLARED = crate::concatcp!(DOMAIN, crate::bus_word::DECLARED),
    }

    /// `yog:key_binding/…`: the key bindings mods declare (provided by the runtime).
    service_key_binding {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, "key_binding", crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, crate::bus_word::DECLARE),
        DECLARED = crate::concatcp!(DOMAIN, crate::bus_word::DECLARED),
    }

    /// `yog:command/…`: the commands mods declare (provided by the runtime).
    service_command {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::yui_vm::COMMAND, crate::sep::PATH),
        DECLARE = crate::concatcp!(DOMAIN, crate::bus_word::DECLARE),
        DECLARED = crate::concatcp!(DOMAIN, crate::bus_word::DECLARED),
    }
}
