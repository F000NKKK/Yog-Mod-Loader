//! An inventory screen: its `<Slot>`s and `<PlayerInventory>` lay out the
//! container the game opens.

use yog_api::ui::{FlexDir, Units, Widget, widget};
use yog_api::SlotLayout;
use yog_kit::{Anchor, Cx, Parts, ScreenMeta, Update, ViewEvent, ViewModel, inventory_of};

#[derive(Default)]
struct Vm;
impl ViewModel for Vm {}

struct Inv;

impl yog_kit::View for Inv {
    type Vm = Vm;
    const META: ScreenMeta = ScreenMeta {
        id: "test:bench",
        w: 176.0,
        h: 166.0,
        anchor: Anchor::Center,
        modal: true,
        pause: false,
    };
    fn build() -> (Self, Widget, Parts) {
        let root = widget::panel(FlexDir::Column)
            .w(Units::Px(176.0))
            .h(Units::Px(166.0))
            .padding(8.0, 8.0, 8.0, 8.0)
            .child(
                widget::panel(FlexDir::Row)
                    .h(Units::Px(60.0))
                    .child(widget::slot(1))
                    .child(widget::spacer().w(Units::Px(10.0)))
                    .child(widget::slot_accepting(0, &["minecraft:redstone", "#c:dusts"])),
            )
            .child(widget::player_inventory());
        (Inv, root, Parts::new())
    }
    fn update(&mut self, _: &mut Widget, _: &Vm, _: &mut Update) {}
    fn dispatch(&mut self, _: &mut Vm, _: &mut Cx<'_, Vm>, _: ViewEvent) {}
}

struct Plain;

impl yog_kit::View for Plain {
    type Vm = Vm;
    const META: ScreenMeta = ScreenMeta {
        id: "test:plain",
        ..Inv::META
    };
    fn build() -> (Self, Widget, Parts) {
        (Plain, widget::panel(FlexDir::Column).child(widget::inv_slot("", 0)), Parts::new())
    }
    fn update(&mut self, _: &mut Widget, _: &Vm, _: &mut Update) {}
    fn dispatch(&mut self, _: &mut Vm, _: &mut Cx<'_, Vm>, _: ViewEvent) {}
}

#[test]
fn slots_come_from_the_layout() {
    let def = inventory_of::<Inv>().unwrap();
    assert_eq!(def.id, "test:bench");
    assert_eq!(def.slot_count, 2);
    // Item corners, 1px inside the frames.
    assert_eq!(
        def.layout,
        [
            SlotLayout { accepts: vec!["minecraft:redstone".into(), "#c:dusts".into()], ..SlotLayout::at(41.0, 9.0) },
            SlotLayout::at(9.0, 9.0),
        ]
    );
    assert!(def.include_player_inventory);
    assert_eq!(def.player_inv_offset, (9.0, 71.0));
    assert_eq!(def.background_size, Some((176.0, 166.0)));
}

#[test]
fn the_player_inventory_is_the_vanilla_grid() {
    let mut ui = yog_api::ui::UiRoot::new("t", widget::player_inventory());
    ui.layout(400.0, 400.0);
    let r = ui.find_rect(widget::PLAYER_INVENTORY_ID).unwrap();
    assert_eq!((r.w, r.h), (9.0 * 18.0, 4.0 * 18.0 + widget::HOTBAR_GAP));
}

#[test]
fn a_screen_without_slots_has_no_inventory() {
    assert!(inventory_of::<Plain>().is_none());
}
