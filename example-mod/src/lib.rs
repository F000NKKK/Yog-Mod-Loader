use yog_api::{Mod, Registry};

mod book;
mod commands;
mod content;
mod events;
mod network;
mod render;

pub struct ExampleMod;

impl Mod for ExampleMod {
    fn register(registry: &mut Registry) {
        content::register(registry);
        events::register(registry);
        commands::register(registry);
        network::register(registry);
        render::register(registry);

        // Register book item right-click behavior (opens YogUIScreen).
        // Rendering is handled by on_ui_render in render.rs (not the JSON book renderer).
        let book = book::guide_book();
        registry.register_book(&book);

        // Announce every 5 minutes (6000 ticks) via the scheduler.
        registry.schedule_repeating(6000, |srv| {
            srv.broadcast("Yog: the server is still running.");
        });
    }
}

yog_api::export_mod!(ExampleMod);
