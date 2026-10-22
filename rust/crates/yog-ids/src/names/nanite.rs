//! Nanites (`docs/design/nanites.md`): the namespaces the game's names come
//! in, the host's mapping table, and the services reflection goes through.

names! {
    /// The namespaces of the game's names.
    mapping_ns {
        /// Mojang's: what nanites name members by.
        MOJANG = "mojang",
        /// Fabric's run-time names (before 26.2).
        INTERMEDIARY = "intermediary",
        /// Forge's run-time member names.
        SRG = "srg",
        /// The game's own obfuscated names.
        OFFICIAL = "official",
        /// Loom's column of Mojang's names in its tiny file.
        NAMED = "named",
    }

    /// The host's mapping table, in its jar.
    mapping_file {
        NAME = crate::concatcp!("yog-mappings", crate::sep::EXT, crate::Format::Data.ext()),
    }

    /// `yog:nanite/…`: reflection — a member resolved once, then called,
    /// read or written by its handle.
    service_nanite {
        DOMAIN = crate::concatcp!(crate::service::PREFIX, crate::nanite_word::NANITE, crate::sep::PATH),
        RESOLVE = crate::concatcp!(DOMAIN, "resolve"),
        CALL = crate::concatcp!(DOMAIN, crate::bus_word::CALL),
        GET = crate::concatcp!(DOMAIN, crate::bus_word::GET),
        SET = crate::concatcp!(DOMAIN, crate::bus_word::SET),
        RELEASE = crate::concatcp!(DOMAIN, crate::bus_word::RELEASE),
    }

    /// What a reference to an object the host holds for a mod is
    /// (`yog_data::Ref`'s kind; its id the object's number).
    nanite_ref {
        OBJECT = crate::concatcp!(crate::ns::YOG, crate::sep::NAMESPACE, "object"),
    }

    /// The words nanites' names are made of.
    nanite_word {
        NANITE = "nanite",
    }

    /// `yog:event/nanite/…`: what nanites' injections emit.
    event_nanite {
        PLANE = crate::concatcp!(crate::event::PREFIX, crate::nanite_word::NANITE, crate::sep::PATH),
        /// An injection point reached; keyed by the point.
        INJECT = crate::concatcp!(PLANE, "inject"),
    }

    /// Where in a method an injection goes: Mixin's `@At` values.
    mixin_at {
        HEAD = "HEAD",
        RETURN = "RETURN",
        TAIL = "TAIL",
        INVOKE = "INVOKE",
        INVOKE_ASSIGN = "INVOKE_ASSIGN",
        FIELD = "FIELD",
        NEW = "NEW",
        CONSTANT = "CONSTANT",
        LOAD = "LOAD",
        STORE = "STORE",
        JUMP = "JUMP",
    }

    /// Mixin's `@At(shift)`.
    mixin_shift {
        NONE = "NONE",
        BEFORE = "BEFORE",
        AFTER = "AFTER",
    }

    /// What joins an injection point's parts in its key
    /// (`<class>#<method><desc>@<at>`).
    nanite_point {
        MEMBER = crate::sep::TAG,
        AT = crate::sep::TYPED,
        TARGET = crate::sep::NAMESPACE,
    }
}
