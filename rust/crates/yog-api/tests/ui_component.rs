//! `ui_component!` registers at load time; `component::build` finds it.

use yog_api::ui::component::{self, Component, Prop, Props};
use yog_api::ui::Widget;
use yog_api::widget;

struct Chip;

impl Component for Chip {
    const NAME: &'static str = "Chip";
    const PROPS: &'static [Prop] = &[Prop::text("label", "chip")];
    fn build(p: &Props) -> Widget {
        widget::label(p.text("label"))
    }
}

struct Pill;

impl Component for Pill {
    const NAME: &'static str = "Pill";
    fn build(_: &Props) -> Widget {
        widget::spacer()
    }
}

yog_api::ui_component!(Chip, Pill);

#[test]
fn registered_on_load() {
    let names: Vec<&str> = component::components().iter().map(|c| c.name).collect();
    assert!(
        names.contains(&"Chip") && names.contains(&"Pill"),
        "{names:?}"
    );
    let w = component::build("Chip", &[("label", "hi")]);
    assert!(matches!(&w.kind, yog_api::ui::widget::WidgetKind::Label(t) if t == "hi"));
}
