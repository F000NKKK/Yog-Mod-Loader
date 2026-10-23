//! `.yparticle` — a particle: its pictures, how it moves and how it's
//! drawn (`yog_registry::Particle`). Its id is its path under
//! `resources/particle/` with `/` as `.` (`resources/particle/conjure.yparticle`
//! is `<mod id>:conjure`). The build writes the game's `particles/<name>.json`
//! and registers the type; the hosts draw it by this, as Yog-IDLE previews it.

/// Where `.yparticle` files live, under `resources/` (with the particles' textures).
pub const DIR: &str = yog_ids::kind::PARTICLE;

/// A particle's id from its file's path under `resources/` and the mod's
/// id; none for a file outside `resources/particle/`.
#[must_use]
pub fn id_of(mod_id: &str, rel: &str) -> Option<String> {
    crate::resource_id(mod_id, rel, DIR, yog_ids::Format::Particle)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_particle_is_named_by_its_path_under_particle() {
        assert_eq!(
            super::id_of("hex", "particle/conjure.yparticle").as_deref(),
            Some("hex:conjure")
        );
        assert_eq!(super::id_of("hex", "particle/cloud.ytexture"), None);
    }
}
