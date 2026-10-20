//! The bus's event addresses (`docs/design/bus.md`); services' are in `services.rs`.

names! {
    /// The bus's events (`docs/design/bus.md`): `yog:event/<plane>/<name>`
    /// after the game did it, `…/pre` before (a cancel stops it).
    event {
        /// What every event address starts with.
        PREFIX = crate::concatcp!(crate::ns::YOG, crate::sep::NAMESPACE, "event", crate::sep::PATH),
        /// What a pre-event's address ends with.
        PRE = crate::concatcp!(crate::sep::PATH, "pre"),
    }

    /// `yog:event/log/…`.
    event_log {
        PLANE = crate::concatcp!(crate::event::PREFIX, "log", crate::sep::PATH),
        RECORD = crate::concatcp!(PLANE, "record"),
    }

    /// `yog:event/player/session/…`.
    event_player_session {
        PLANE = crate::concatcp!(crate::event::PREFIX, "player", crate::sep::PATH, "session", crate::sep::PATH),
        JOIN = crate::concatcp!(PLANE, "join"),
        LEAVE = crate::concatcp!(PLANE, "leave"),
    }

    /// `yog:event/player/life/…`.
    event_player_life {
        PLANE = crate::concatcp!(crate::event::PREFIX, "player", crate::sep::PATH, "life", crate::sep::PATH),
        DEATH = crate::concatcp!(PLANE, "death"),
        DEATH_PRE = crate::concatcp!(DEATH, crate::event::PRE),
        RESPAWN = crate::concatcp!(PLANE, "respawn"),
    }

    /// `yog:event/player/chat/…`.
    event_player_chat {
        PLANE = crate::concatcp!(crate::event::PREFIX, "player", crate::sep::PATH, "chat", crate::sep::PATH),
        MESSAGE = crate::concatcp!(PLANE, "message"),
        MESSAGE_PRE = crate::concatcp!(MESSAGE, crate::event::PRE),
    }

    /// `yog:event/player/action/…`.
    event_player_action {
        PLANE = crate::concatcp!(crate::event::PREFIX, "player", crate::sep::PATH, "action", crate::sep::PATH),
        MOVE = crate::concatcp!(PLANE, "move"),
        USE_ITEM = crate::concatcp!(PLANE, "use_item"),
        USE_ITEM_PRE = crate::concatcp!(USE_ITEM, crate::event::PRE),
        ATTACK = crate::concatcp!(PLANE, "attack"),
        INTERACT = crate::concatcp!(PLANE, "interact"),
        INTERACT_PRE = crate::concatcp!(INTERACT, crate::event::PRE),
        CRAFT = crate::concatcp!(PLANE, "craft"),
        PICKUP = crate::concatcp!(PLANE, "pickup"),
        PICKUP_PRE = crate::concatcp!(PICKUP, crate::event::PRE),
    }

    /// `yog:event/player/progress/…`.
    event_player_progress {
        PLANE = crate::concatcp!(crate::event::PREFIX, "player", crate::sep::PATH, "progress", crate::sep::PATH),
        ADVANCEMENT = crate::concatcp!(PLANE, "advancement"),
    }

    /// `yog:event/block/…`.
    event_block {
        PLANE = crate::concatcp!(crate::event::PREFIX, "block", crate::sep::PATH),
        BREAK = crate::concatcp!(PLANE, "break"),
        BREAK_PRE = crate::concatcp!(BREAK, crate::event::PRE),
        PLACE = crate::concatcp!(PLANE, "place"),
        PLACE_PRE = crate::concatcp!(PLACE, crate::event::PRE),
        USE = crate::concatcp!(PLANE, "use"),
        USE_PRE = crate::concatcp!(USE, crate::event::PRE),
    }

    /// `yog:event/entity/life/…`.
    event_entity_life {
        PLANE = crate::concatcp!(crate::event::PREFIX, "entity", crate::sep::PATH, "life", crate::sep::PATH),
        SPAWN = crate::concatcp!(PLANE, "spawn"),
        SPAWN_PRE = crate::concatcp!(SPAWN, crate::event::PRE),
        DEATH = crate::concatcp!(PLANE, "death"),
    }

    /// `yog:event/entity/combat/…`.
    event_entity_combat {
        PLANE = crate::concatcp!(crate::event::PREFIX, "entity", crate::sep::PATH, "combat", crate::sep::PATH),
        DAMAGE = crate::concatcp!(PLANE, "damage"),
        DAMAGE_PRE = crate::concatcp!(DAMAGE, crate::event::PRE),
        PROJECTILE_HIT = crate::concatcp!(PLANE, "projectile_hit"),
        PROJECTILE_HIT_PRE = crate::concatcp!(PROJECTILE_HIT, crate::event::PRE),
    }

    /// `yog:event/entity/game/…`; keyed by the entity's type.
    event_entity_game {
        PLANE = crate::concatcp!(crate::event::PREFIX, "entity", crate::sep::PATH, "game", crate::sep::PATH),
        TICK = crate::concatcp!(PLANE, "tick"),
    }

    /// `yog:event/world/…`.
    event_world {
        PLANE = crate::concatcp!(crate::event::PREFIX, "world", crate::sep::PATH),
        EXPLOSION = crate::concatcp!(PLANE, "explosion"),
        EXPLOSION_PRE = crate::concatcp!(EXPLOSION, crate::event::PRE),
    }

    /// `yog:event/world/generation/…`; keyed by the generator type.
    event_world_generation {
        PLANE = crate::concatcp!(crate::event::PREFIX, "world", crate::sep::PATH, "generation", crate::sep::PATH),
        CHUNK = crate::concatcp!(PLANE, "chunk"),
    }

    /// `yog:event/world/loot/…`.
    event_world_loot {
        PLANE = crate::concatcp!(crate::event::PREFIX, "world", crate::sep::PATH, "loot", crate::sep::PATH),
        ROLL = crate::concatcp!(PLANE, "roll"),
    }

    /// `yog:event/container/…`.
    event_container {
        PLANE = crate::concatcp!(crate::event::PREFIX, "container", crate::sep::PATH),
        OPEN = crate::concatcp!(PLANE, "open"),
        OPEN_PRE = crate::concatcp!(OPEN, crate::event::PRE),
        CLOSE = crate::concatcp!(PLANE, "close"),
    }

    /// `yog:event/container/machine/…`.
    event_container_machine {
        PLANE = crate::concatcp!(crate::event::PREFIX, "container", crate::sep::PATH, "machine", crate::sep::PATH),
        CRAFT = crate::concatcp!(PLANE, "craft"),
        CRAFT_PRE = crate::concatcp!(CRAFT, crate::event::PRE),
    }

    /// `yog:event/server/game/…`.
    event_server_game {
        PLANE = crate::concatcp!(crate::event::PREFIX, "server", crate::sep::PATH, "game", crate::sep::PATH),
        TICK = crate::concatcp!(PLANE, "tick"),
    }

    /// `yog:event/server/life/…`.
    event_server_life {
        PLANE = crate::concatcp!(crate::event::PREFIX, "server", crate::sep::PATH, "life", crate::sep::PATH),
        STARTED = crate::concatcp!(PLANE, "started"),
        STOPPING = crate::concatcp!(PLANE, "stopping"),
    }

    /// `yog:event/server/command/…`; keyed by the command's name.
    event_server_command {
        PLANE = crate::concatcp!(crate::event::PREFIX, "server", crate::sep::PATH, "command", crate::sep::PATH),
        RUN = crate::concatcp!(PLANE, "run"),
    }

    /// `yog:event/server/net/…`; keyed by channel.
    event_server_net {
        PLANE = crate::concatcp!(crate::event::PREFIX, "server", crate::sep::PATH, "net", crate::sep::PATH),
        PACKET = crate::concatcp!(PLANE, "packet"),
    }

    /// `yog:event/client/game/…`.
    event_client_game {
        PLANE = crate::concatcp!(crate::event::PREFIX, "client", crate::sep::PATH, "game", crate::sep::PATH),
        TICK = crate::concatcp!(PLANE, "tick"),
        TRAVEL = crate::concatcp!(PLANE, "travel"),
    }

    /// `yog:event/client/input/…`.
    event_client_input {
        PLANE = crate::concatcp!(crate::event::PREFIX, "client", crate::sep::PATH, "input", crate::sep::PATH),
        KEY = crate::concatcp!(PLANE, "key"),
        KEY_PRE = crate::concatcp!(KEY, crate::event::PRE),
        BINDING = crate::concatcp!(PLANE, "binding"),
    }

    /// `yog:event/client/ui/…`.
    event_client_ui {
        PLANE = crate::concatcp!(crate::event::PREFIX, "client", crate::sep::PATH, "ui", crate::sep::PATH),
        OPEN = crate::concatcp!(PLANE, "open"),
        CLOSE = crate::concatcp!(PLANE, "close"),
        INPUT = crate::concatcp!(PLANE, "input"),
        INPUT_PRE = crate::concatcp!(INPUT, crate::event::PRE),
    }

    /// `yog:event/client/render/…`.
    event_client_render {
        PLANE = crate::concatcp!(crate::event::PREFIX, "client", crate::sep::PATH, "render", crate::sep::PATH),
        HUD = crate::concatcp!(PLANE, "hud"),
        WORLD = crate::concatcp!(PLANE, "world"),
        UI = crate::concatcp!(PLANE, "ui"),
        TOOLTIP = crate::concatcp!(PLANE, "tooltip"),
        PAGE = crate::concatcp!(PLANE, "page"),
        CAMERA = crate::concatcp!(PLANE, "camera"),
        BODY = crate::concatcp!(PLANE, "body"),
    }

    /// `yog:event/client/net/…`; keyed by channel.
    event_client_net {
        PLANE = crate::concatcp!(crate::event::PREFIX, "client", crate::sep::PATH, "net", crate::sep::PATH),
        PACKET = crate::concatcp!(PLANE, "packet"),
    }
}
