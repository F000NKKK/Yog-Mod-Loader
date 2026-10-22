//! Mirrors (`docs/design/bus.md`, "Mirrors"): facts, the key spaces they're
//! kept by, and the services that declare and provide them.

names! {
    /// The words mirrors' names are made of.
    mirror_word {
        MIRROR = "mirror",
        FACT = "fact",
        KEYS = "keys",
        POS = "pos",
        VELOCITY = "velocity",
        HEALTH = "health",
        UUID = "uuid",
        /// A block's: can be stood on and can't be walked through.
        SOLID = "solid",
        /// A block's: water, lava…
        LIQUID = "liquid",
        /// A block's: ladders, vines.
        CLIMBABLE = "climbable",
        /// A block's: hurts to touch (fire, lava, cactus…).
        HARMFUL = "harmful",
        /// An entity's: its eyes' height over its feet.
        EYE_HEIGHT = "eye_height",
        /// An entity's: a monster (the game's `Enemy`).
        HOSTILE = "hostile",
        /// A block's: its collision is the whole cube.
        FULL = "full",
        /// A block's: falls when nothing holds it (sand, gravel…).
        FALLS = "falls",
        /// A block's: how hard it is to break (negative: it can't be).
        HARDNESS = "hardness",
    }

    /// `yog:mirror/…`: declaring a mirror, providing one, and the mirrors
    /// themselves (`yog:mirror/<mod>/<name>`, each one's writer).
    service_mirror {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::mirror_word::MIRROR, crate::sep::PATH),
        /// A declaration: its mirror's directory and the columns asked for.
        DECLARE = crate::concatcp!(DOMAIN, crate::bus_word::DECLARE),
        /// A declaration let go: what only it asked for stops being written.
        RELEASE = crate::concatcp!(DOMAIN, crate::bus_word::RELEASE),
        /// A provider: who writes a mirror's columns (its writer answers the
        /// mirror's own address).
        PROVIDE = crate::concatcp!(DOMAIN, "provide"),
        /// A provider saying its on-change mirror changed.
        TOUCH = crate::concatcp!(DOMAIN, "touch"),
        /// Keys coming into a key space and leaving it.
        KEYS = crate::concatcp!(DOMAIN, crate::mirror_word::KEYS),
    }

    /// The host's own mirrors: `yog:mirror/yog/<name>`.
    host_mirror {
        PREFIX = crate::concatcp!(crate::service_mirror::DOMAIN, crate::ns::YOG, crate::sep::PATH),
        /// Every live entity.
        ENTITY = crate::concatcp!(PREFIX, crate::kind::ENTITY),
        /// Every player (part of the entities).
        PLAYER = crate::concatcp!(PREFIX, crate::word::PLAYER),
        /// The blocks of the areas asked for, a dimension each
        /// (`…/block/<dimension>`).
        BLOCK = crate::concatcp!(PREFIX, crate::word::BLOCK),
        /// What a client sees (`…/client/…`): written after its ticks, on
        /// any server.
        CLIENT = crate::concatcp!(PREFIX, crate::bus_word::CLIENT, crate::sep::PATH),
        /// The entities the client sees.
        CLIENT_ENTITY = crate::concatcp!(CLIENT, crate::kind::ENTITY),
        /// The blocks of the areas asked for, in the client's world.
        CLIENT_BLOCK = crate::concatcp!(CLIENT, crate::word::BLOCK),
    }

    /// The key spaces columns are kept by: `yog:keys/<space>`.
    key_space {
        PREFIX = crate::concatcp!(crate::ns::YOG, crate::sep::NAMESPACE, crate::mirror_word::KEYS, crate::sep::PATH),
        /// Live entities, by network id.
        ENTITY = crate::concatcp!(PREFIX, crate::kind::ENTITY),
        /// Players, by their entity's network id; part of [`ENTITY`].
        PLAYER = crate::concatcp!(PREFIX, crate::word::PLAYER),
        /// Blocks, by packed position (`BlockPos::packed`): a family, one
        /// space a dimension (`yog:keys/block/<dimension>`) — the packed
        /// position fills the key.
        BLOCK = crate::concatcp!(PREFIX, crate::word::BLOCK),
        /// What a client sees: its own spaces (one world at a time).
        CLIENT = crate::concatcp!(PREFIX, crate::bus_word::CLIENT, crate::sep::PATH),
        /// The entities the client sees, by network id.
        CLIENT_ENTITY = crate::concatcp!(CLIENT, crate::kind::ENTITY),
        /// The client world's blocks, by packed position.
        CLIENT_BLOCK = crate::concatcp!(CLIENT, crate::word::BLOCK),
    }

    /// Facts: one thing known about a key (`yog:fact/<space>/<what>`). A
    /// column is one fact in one key space; every mirror field naming the
    /// same fact links to the same column.
    fact {
        PREFIX = crate::concatcp!(crate::ns::YOG, crate::sep::NAMESPACE, crate::mirror_word::FACT, crate::sep::PATH),
        ENTITY = crate::concatcp!(PREFIX, crate::kind::ENTITY, crate::sep::PATH),
        /// Where it is (`Point`).
        ENTITY_POS = crate::concatcp!(ENTITY, crate::mirror_word::POS),
        /// How fast it moves (`Point`, blocks a tick).
        ENTITY_VELOCITY = crate::concatcp!(ENTITY, crate::mirror_word::VELOCITY),
        /// Where it looks (`[yaw, pitch]`, degrees).
        ENTITY_ROTATION = crate::concatcp!(ENTITY, crate::model::ROTATION),
        /// Its type (an atom).
        ENTITY_KIND = crate::concatcp!(ENTITY, crate::word::KIND),
        /// The dimension it's in (an atom).
        ENTITY_DIMENSION = crate::concatcp!(ENTITY, crate::word::DIMENSION),
        /// Its health (living ones; others 0).
        ENTITY_HEALTH = crate::concatcp!(ENTITY, crate::mirror_word::HEALTH),
        /// Its uuid (`[high, low]`): how mods know it elsewhere.
        ENTITY_UUID = crate::concatcp!(ENTITY, crate::mirror_word::UUID),
        PLAYER = crate::concatcp!(PREFIX, crate::word::PLAYER, crate::sep::PATH),
        /// Its name (an atom).
        PLAYER_NAME = crate::concatcp!(PLAYER, crate::word::NAME),
        BLOCK = crate::concatcp!(PREFIX, crate::word::BLOCK, crate::sep::PATH),
        /// Its state (an atom: `minecraft:stone`, properties included).
        BLOCK_STATE = crate::concatcp!(BLOCK, crate::word::STATE),
        CLIENT_ENTITY = crate::concatcp!(PREFIX, crate::bus_word::CLIENT, crate::sep::PATH, crate::kind::ENTITY, crate::sep::PATH),
        CLIENT_ENTITY_POS = crate::concatcp!(CLIENT_ENTITY, crate::mirror_word::POS),
        CLIENT_ENTITY_VELOCITY = crate::concatcp!(CLIENT_ENTITY, crate::mirror_word::VELOCITY),
        CLIENT_ENTITY_ROTATION = crate::concatcp!(CLIENT_ENTITY, crate::model::ROTATION),
        CLIENT_ENTITY_KIND = crate::concatcp!(CLIENT_ENTITY, crate::word::KIND),
        CLIENT_ENTITY_HEALTH = crate::concatcp!(CLIENT_ENTITY, crate::mirror_word::HEALTH),
        CLIENT_ENTITY_UUID = crate::concatcp!(CLIENT_ENTITY, crate::mirror_word::UUID),
        CLIENT_ENTITY_EYE_HEIGHT = crate::concatcp!(CLIENT_ENTITY, crate::mirror_word::EYE_HEIGHT),
        CLIENT_ENTITY_HOSTILE = crate::concatcp!(CLIENT_ENTITY, crate::mirror_word::HOSTILE),
        CLIENT_ENTITY_NAME = crate::concatcp!(CLIENT_ENTITY, crate::word::NAME),
        CLIENT_BLOCK = crate::concatcp!(PREFIX, crate::bus_word::CLIENT, crate::sep::PATH, crate::word::BLOCK, crate::sep::PATH),
        CLIENT_BLOCK_STATE = crate::concatcp!(CLIENT_BLOCK, crate::word::STATE),
        CLIENT_BLOCK_SOLID = crate::concatcp!(CLIENT_BLOCK, crate::mirror_word::SOLID),
        CLIENT_BLOCK_LIQUID = crate::concatcp!(CLIENT_BLOCK, crate::mirror_word::LIQUID),
        CLIENT_BLOCK_CLIMBABLE = crate::concatcp!(CLIENT_BLOCK, crate::mirror_word::CLIMBABLE),
        CLIENT_BLOCK_HARMFUL = crate::concatcp!(CLIENT_BLOCK, crate::mirror_word::HARMFUL),
        CLIENT_BLOCK_FULL = crate::concatcp!(CLIENT_BLOCK, crate::mirror_word::FULL),
        CLIENT_BLOCK_FALLS = crate::concatcp!(CLIENT_BLOCK, crate::mirror_word::FALLS),
        CLIENT_BLOCK_HARDNESS = crate::concatcp!(CLIENT_BLOCK, crate::mirror_word::HARDNESS),
    }

    /// What's wrong with a mirror's declaration or provider (`YM…`).
    mirror_problem {
        /// A request that failed otherwise.
        FAILED = "YM0000",
        /// A key space no one made.
        UNKNOWN_SPACE = "YM0001",
        /// A field shaped otherwise than its fact's column already is.
        WRONG_SHAPE = "YM0002",
        /// A fact of a space the mirror's isn't part of.
        FOREIGN_FACT = "YM0003",
        /// More fields than a mask holds.
        TOO_MANY_FIELDS = "YM0004",
        /// A mirror someone else provides, or declared otherwise before.
        TAKEN = "YM0005",
    }
}
