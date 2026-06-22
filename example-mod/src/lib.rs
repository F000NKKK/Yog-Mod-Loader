use yog_api::{Mod, Registry};

mod commands;
mod content;
mod events;
mod network;

pub struct ExampleMod;

impl Mod for ExampleMod {
    fn register(registry: &mut Registry) {
        content::register(registry);
        events::register(registry);
        commands::register(registry);
        network::register(registry);
    }
}

yog_api::export_mod!(ExampleMod);
