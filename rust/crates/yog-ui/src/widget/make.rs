//! The widgets, made: a constructor per kind.

use super::{
    Image, SLOT_SIZE, Stretch, TEXT_BOX_PAD_X, TEXT_BOX_TEXT, TextBox, Widget, WidgetKind,
};
use yog_gfx::Color;

use crate::layout::FlexDir;

pub fn panel(dir: FlexDir) -> Widget {
    Widget::new(WidgetKind::Panel(dir))
}
/// WPF's `Grid`: a container of rows and columns — give it [`Widget::rows`]
/// and [`Widget::cols`] ([`Track`]s: pixels, `Auto` or `*` shares) and put
/// each child in a cell with [`Widget::cell`] / [`Widget::span`]. A child
/// fills its cell unless it has [`Widget::halign`] / [`Widget::valign`] (or
/// an explicit size); [`Widget::margin`] insets it.
///
/// ```
/// use yog_ui::widget::{self, GridLength, Track};
/// let ui = widget::grid()
///     .rows([Track::auto(), Track::star(1.0), Track::px(20.0)])
///     .cols([GridLength::Px(80.0), GridLength::STAR])
///     .child(widget::label("Title").cell(0, 0).span(1, 2))
///     .child(widget::scroll_view().cell(1, 0))
///     .child(widget::label("Details").cell(1, 1))
///     .child(widget::button("OK").cell(2, 1));
/// ```
pub fn grid() -> Widget {
    Widget::new(WidgetKind::Grid(Box::default()))
}
/// A vertically scrolling column: clips its children, scrolls them by
/// [`Widget::scroll_y`] and draws a scrollbar when they overflow — WPF's
/// `ScrollViewer` around a `StackPanel`. Give it a fixed (or flex) height.
pub fn scroll_view() -> Widget {
    panel(FlexDir::Column).clip(true).scrollbar(true)
}
pub fn label(text: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::Label(text.into()))
}
pub fn button(text: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::Button(text.into()))
}
pub fn item_slot(item_id: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::ItemSlot(item_id.into()))
}
/// An inventory-slot decoration (icon + stack-count overlay) for `item_id`
/// (empty string = empty slot) with `count` items. Purely decorative — real
/// drag-and-drop still goes through the vanilla `Slot` this overlays.
pub fn inv_slot(item_id: impl Into<String>, count: u32) -> Widget {
    Widget::new(WidgetKind::InvSlot {
        item_id: item_id.into(),
        count,
    })
    .w(SLOT_SIZE)
    .h(SLOT_SIZE)
}
use crate::inventory_ids::PLAYER_INVENTORY;

/// Marks an inventory screen as a machine making recipes of `kind`: its
/// `places` (`(kind's slot, index, inventory slot)`) and its `result` slot
/// — what a `.yui` `<RecipeArea>` becomes. Draws nothing.
pub fn machine(kind: &str, places: &[(&str, usize, usize)], result: usize) -> Widget {
    spacer()
        .w(0.0)
        .h(0.0)
        .id(crate::inventory_ids::MachineId::write(kind, places, result))
}
/// Gap between the player's main inventory and their hotbar (vanilla).
pub const HOTBAR_GAP: f32 = 4.0;

/// Slot `index` of an inventory screen's container: the frame its real
/// slot sits in — the game draws the item, the hover and the drag over it.
/// Where it lands in the layout is where the slot is.
pub fn slot(index: usize) -> Widget {
    inv_slot("", 0).id(crate::inventory_ids::slot_id(index, &[]))
}
/// A [`slot`] that only takes `accepts`: item ids (`minecraft:redstone`)
/// and item tags (`#c:dusts`).
pub fn slot_accepting(index: usize, accepts: &[&str]) -> Widget {
    inv_slot("", 0).id(crate::inventory_ids::slot_id(index, accepts))
}
/// The player's inventory on an inventory screen: 3 rows of 9, then the
/// hotbar [`HOTBAR_GAP`] below — frames for the real slots, like [`slot`].
pub fn player_inventory() -> Widget {
    let row = || {
        (0..9).fold(panel(FlexDir::Row).gap(0.0), |r, _| {
            r.child(inv_slot("", 0))
        })
    };
    panel(FlexDir::Column)
        .id(PLAYER_INVENTORY)
        .gap(0.0)
        .child(row())
        .child(row())
        .child(row())
        .child(row().margin(HOTBAR_GAP, 0.0, 0.0, 0.0))
}
/// An image from a Yog image source — `"item:apple"`, `"block:stone"`,
/// `"sprite:button"`, `"mymod:gui/logo"` (see [`yog_gfx::image`]) — the
/// host resolves it for its Minecraft version. Sized to the image's natural
/// size (16×16 until the host told it) unless given `w`/`h`; scaled into the
/// widget per [`Widget::stretch`] (default `Uniform`), nearest-neighbour.
pub fn image(source: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::Image(Image {
        source: source.into(),
        stretch: Stretch::Uniform,
        region: None,
        tint: Color::WHITE,
    }))
}
pub fn spacer() -> Widget {
    Widget::new(WidgetKind::Spacer)
}
/// A box drawn by the code — the [`Surface`](crate::Surface) registered
/// under `id` with [`crate::surface::register`]. No natural size: give it
/// `w`/`h` (or let a parent stretch it).
pub fn surface(id: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::Surface(id.into()))
}
/// A single-line text input holding `text` — black box, grey border (white
/// while focused), like vanilla's `EditBox`. Its text, caret and focus live
/// in a [`TextBoxState`](crate::TextBoxState) that the code-behind keeps and
/// applies to it each frame.
pub fn text_box(text: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::TextBox(TextBox {
        text: text.into(),
        ..TextBox::default()
    }))
    .bg(Color::BLACK)
    .color(TEXT_BOX_TEXT)
    .padding(0.0, TEXT_BOX_PAD_X, 0.0, TEXT_BOX_PAD_X)
}
