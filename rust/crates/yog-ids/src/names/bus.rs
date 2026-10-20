//! The bus's addresses (`docs/design/bus.md`): events and services.

names! {

    /// The bus's events (`docs/design/bus.md`): `yog:event/<plane>/<name>`
    /// after the game did it, `…/pre` before (a cancel stops it).
    event {
        /// What every event address starts with.
        PREFIX = concatcp!(ns::YOG, sep::NAMESPACE, "event", sep::PATH),
        /// What a pre-event's address ends with.
        PRE = concatcp!(sep::PATH, "pre"),
    }

    /// The bus's services (`docs/design/bus.md`): `yog:<domain>/<name>`,
    /// answered by whoever provides them.
    service {
        /// What every service address starts with.
        PREFIX = concatcp!(ns::YOG, sep::NAMESPACE),
    }

    /// `yog:block/…`.
    service_block {
        DOMAIN = concatcp!(service::PREFIX, "block", sep::PATH),
        INSERT = concatcp!(DOMAIN, "insert"),
    }

    /// `yog:item/…`.
    service_item {
        DOMAIN = concatcp!(service::PREFIX, "item", sep::PATH),
        TEXT = concatcp!(DOMAIN, "text"),
        PICTURE = concatcp!(DOMAIN, "picture"),
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

    /// `yog:event/player/session/…`.
    event_player_session {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "session", sep::PATH),
        JOIN = concatcp!(PLANE, "join"),
        LEAVE = concatcp!(PLANE, "leave"),
    }

    /// `yog:event/player/life/…`.
    event_player_life {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "life", sep::PATH),
        DEATH = concatcp!(PLANE, "death"),
        DEATH_PRE = concatcp!(DEATH, event::PRE),
        RESPAWN = concatcp!(PLANE, "respawn"),
    }

    /// `yog:event/player/chat/…`.
    event_player_chat {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "chat", sep::PATH),
        MESSAGE = concatcp!(PLANE, "message"),
        MESSAGE_PRE = concatcp!(MESSAGE, event::PRE),
    }

    /// `yog:event/player/action/…`.
    event_player_action {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "action", sep::PATH),
        MOVE = concatcp!(PLANE, "move"),
        USE_ITEM = concatcp!(PLANE, "use_item"),
        USE_ITEM_PRE = concatcp!(USE_ITEM, event::PRE),
        ATTACK = concatcp!(PLANE, "attack"),
        INTERACT = concatcp!(PLANE, "interact"),
        INTERACT_PRE = concatcp!(INTERACT, event::PRE),
        CRAFT = concatcp!(PLANE, "craft"),
        PICKUP = concatcp!(PLANE, "pickup"),
        PICKUP_PRE = concatcp!(PICKUP, event::PRE),
    }

    /// `yog:event/player/progress/…`.
    event_player_progress {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "progress", sep::PATH),
        ADVANCEMENT = concatcp!(PLANE, "advancement"),
    }

    /// `yog:event/block/…`.
    event_block {
        PLANE = concatcp!(event::PREFIX, "block", sep::PATH),
        BREAK = concatcp!(PLANE, "break"),
        BREAK_PRE = concatcp!(BREAK, event::PRE),
        PLACE = concatcp!(PLANE, "place"),
        PLACE_PRE = concatcp!(PLACE, event::PRE),
        USE = concatcp!(PLANE, "use"),
        USE_PRE = concatcp!(USE, event::PRE),
    }

    /// `yog:event/entity/life/…`.
    event_entity_life {
        PLANE = concatcp!(event::PREFIX, "entity", sep::PATH, "life", sep::PATH),
        SPAWN = concatcp!(PLANE, "spawn"),
        SPAWN_PRE = concatcp!(SPAWN, event::PRE),
        DEATH = concatcp!(PLANE, "death"),
    }

    /// `yog:event/entity/combat/…`.
    event_entity_combat {
        PLANE = concatcp!(event::PREFIX, "entity", sep::PATH, "combat", sep::PATH),
        DAMAGE = concatcp!(PLANE, "damage"),
        DAMAGE_PRE = concatcp!(DAMAGE, event::PRE),
        PROJECTILE_HIT = concatcp!(PLANE, "projectile_hit"),
        PROJECTILE_HIT_PRE = concatcp!(PROJECTILE_HIT, event::PRE),
    }

    /// `yog:event/entity/game/…`; keyed by the entity's type.
    event_entity_game {
        PLANE = concatcp!(event::PREFIX, "entity", sep::PATH, "game", sep::PATH),
        TICK = concatcp!(PLANE, "tick"),
    }

    /// `yog:event/world/…`.
    event_world {
        PLANE = concatcp!(event::PREFIX, "world", sep::PATH),
        EXPLOSION = concatcp!(PLANE, "explosion"),
        EXPLOSION_PRE = concatcp!(EXPLOSION, event::PRE),
    }

    /// `yog:event/world/generation/…`; keyed by the generator type.
    event_world_generation {
        PLANE = concatcp!(event::PREFIX, "world", sep::PATH, "generation", sep::PATH),
        CHUNK = concatcp!(PLANE, "chunk"),
    }

    /// `yog:event/world/loot/…`.
    event_world_loot {
        PLANE = concatcp!(event::PREFIX, "world", sep::PATH, "loot", sep::PATH),
        ROLL = concatcp!(PLANE, "roll"),
    }

    /// `yog:event/container/…`.
    event_container {
        PLANE = concatcp!(event::PREFIX, "container", sep::PATH),
        OPEN = concatcp!(PLANE, "open"),
        OPEN_PRE = concatcp!(OPEN, event::PRE),
        CLOSE = concatcp!(PLANE, "close"),
    }

    /// `yog:event/container/machine/…`.
    event_container_machine {
        PLANE = concatcp!(event::PREFIX, "container", sep::PATH, "machine", sep::PATH),
        CRAFT = concatcp!(PLANE, "craft"),
        CRAFT_PRE = concatcp!(CRAFT, event::PRE),
    }

    /// `yog:event/server/game/…`.
    event_server_game {
        PLANE = concatcp!(event::PREFIX, "server", sep::PATH, "game", sep::PATH),
        TICK = concatcp!(PLANE, "tick"),
    }

    /// `yog:event/server/life/…`.
    event_server_life {
        PLANE = concatcp!(event::PREFIX, "server", sep::PATH, "life", sep::PATH),
        STARTED = concatcp!(PLANE, "started"),
        STOPPING = concatcp!(PLANE, "stopping"),
    }

    /// `yog:event/server/command/…`; keyed by the command's name.
    event_server_command {
        PLANE = concatcp!(event::PREFIX, "server", sep::PATH, "command", sep::PATH),
        RUN = concatcp!(PLANE, "run"),
    }

    /// `yog:event/server/net/…`; keyed by channel.
    event_server_net {
        PLANE = concatcp!(event::PREFIX, "server", sep::PATH, "net", sep::PATH),
        PACKET = concatcp!(PLANE, "packet"),
    }

    /// `yog:event/client/game/…`.
    event_client_game {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "game", sep::PATH),
        TICK = concatcp!(PLANE, "tick"),
        TRAVEL = concatcp!(PLANE, "travel"),
    }

    /// `yog:event/client/input/…`.
    event_client_input {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "input", sep::PATH),
        KEY = concatcp!(PLANE, "key"),
        KEY_PRE = concatcp!(KEY, event::PRE),
        BINDING = concatcp!(PLANE, "binding"),
    }

    /// `yog:event/client/ui/…`.
    event_client_ui {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "ui", sep::PATH),
        OPEN = concatcp!(PLANE, "open"),
        CLOSE = concatcp!(PLANE, "close"),
        INPUT = concatcp!(PLANE, "input"),
        INPUT_PRE = concatcp!(INPUT, event::PRE),
    }

    /// `yog:event/client/render/…`.
    event_client_render {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "render", sep::PATH),
        HUD = concatcp!(PLANE, "hud"),
        WORLD = concatcp!(PLANE, "world"),
        UI = concatcp!(PLANE, "ui"),
        TOOLTIP = concatcp!(PLANE, "tooltip"),
        PAGE = concatcp!(PLANE, "page"),
        CAMERA = concatcp!(PLANE, "camera"),
        BODY = concatcp!(PLANE, "body"),
    }

    /// `yog:event/client/net/…`; keyed by channel.
    event_client_net {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "net", sep::PATH),
        PACKET = concatcp!(PLANE, "packet"),
    }
}
