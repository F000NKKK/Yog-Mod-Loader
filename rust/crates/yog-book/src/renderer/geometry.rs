//! Where things are on a book, in its own pixels (Patchouli's layout, so
//! its books and sheets look as they do there): the sheet, the pages, the
//! parts of each kind of page.

/// The open book: the sprite sheet's top-left 272×180.
pub const BK_W: f32 = 272.0;
pub const BK_H: f32 = 180.0;
/// A page, and where the two sit in the book (both under [`TOP_PAD`]).
pub const PAGE_W: f32 = 116.0;
pub const PAGE_H: f32 = 156.0;
pub const TOP_PAD: f32 = 18.0;
pub const LEFT_X: f32 = 15.0;
pub const RIGHT_X: f32 = 141.0;
/// The separator strip on the sheet.
pub const SEP_U: f32 = 140.0;
pub const SEP_V: f32 = 180.0;
pub const SEP_W: f32 = 110.0;
pub const SEP_H: f32 = 3.0;

/// Room (GUI px) the book keeps free around it before it scales down, and
/// the smallest it scales to.
pub const SCREEN_MARGIN: f32 = 16.0;
pub const MIN_SCALE: f32 = 0.4;
/// How much of the screen's height the book takes (as at GUI scale 4 on a
/// 1080p screen), whatever the GUI scale.
pub const SCREEN_FILL: f32 = 0.67;

/// A page's header: its title's height, the separator under it, the body under that.
pub const TITLE_H: f32 = 9.0;
pub const SEP_Y: f32 = 12.0;
pub const BODY_Y: f32 = 22.0;
/// A recipe's, spotlight's or picture's title height.
pub const PICTURE_TITLE_H: f32 = 10.0;
/// The room a picture's title takes, gap included.
pub const PICTURE_TITLE_ROOM: f32 = 14.0;
/// The gap under a picture, a text, a spacer.
pub const GAP: f32 = 4.0;
/// A title's inset from the page's sides.
pub const TITLE_INSET: f32 = 10.0;

/// The text's inset from the page's sides.
pub const TEXT_INSET: f32 = 8.0;
/// The page navigation's row (back, n/m, home, next).
pub const NAV_H: f32 = 12.0;
/// The navigation's arrows on the sheet: next, back (curved, 18×10) and
/// home (straight, 18×9) — Patchouli's `GuiButtonBookArrow` / `GuiButtonBookBack`.
pub const ARROW_U: f32 = 272.0;
pub const ARROW_NEXT_V: f32 = 0.0;
pub const ARROW_BACK_V: f32 = 10.0;
pub const ARROW_W: f32 = 18.0;
pub const ARROW_H: f32 = 10.0;
pub const HOME_U: f32 = 308.0;
pub const HOME_V: f32 = 0.0;
pub const HOME_H: f32 = 9.0;

/// The nameplate on the landing page: where on the sheet, where on the book.
pub const NAMEPLATE_V: f32 = 180.0;
pub const NAMEPLATE_W: f32 = 140.0;
pub const NAMEPLATE_H: f32 = 31.0;
pub const NAMEPLATE_X: f32 = LEFT_X - 8.0;
pub const NAMEPLATE_Y: f32 = 12.0;
/// The book's name and its second line on the nameplate.
pub const BOOK_NAME_X: f32 = 13.0;
pub const BOOK_NAME_Y: f32 = 16.0;
pub const EDITION_X: f32 = 24.0;
pub const EDITION_Y: f32 = 24.0;
/// The second line is drawn smaller than the name (Patchouli's 0.6).
pub const EDITION_SCALE: f32 = 0.6;
/// Where the landing text starts (the entry count is in the navigation's row under it).
pub const LANDING_Y: f32 = 25.0;
/// The landing text shrinks this far at most, this much at a time.
pub const LANDING_MIN_FIT: f32 = 0.6;
pub const LANDING_FIT_STEP: f32 = 0.05;

/// The category grid: columns, a cell, where it starts on the page, and an
/// icon's inset in its cell.
pub const CAT_COLUMNS: usize = 4;
pub const CAT_CELL: f32 = 24.0;
pub const CAT_GRID_X: f32 = 10.0;
pub const CAT_GRID_Y: f32 = 25.0;
pub const CAT_ICON_INSET: f32 = 4.0;

/// The entry list: where it starts, a row, an icon (half size) and its
/// inset, the list's bottom padding.
pub const LIST_Y: f32 = 20.0;
pub const LIST_ROW_H: f32 = 11.0;
pub const LIST_ICON: f32 = 8.0;
pub const LIST_ICON_INSET: f32 = 1.0;
pub const LIST_PAD_BOTTOM: f32 = 8.0;
/// Room a name has in its row (the page less the icon and paddings).
pub const LIST_NAME_W: f32 = PAGE_W - 26.0;

/// A spotlight: its frame on the crafting sheet and on the page, its item.
pub const SPOT_FRAME_V: f32 = 102.0;
pub const SPOT_FRAME_W: f32 = 66.0;
pub const SPOT_FRAME_H: f32 = 26.0;
pub const SPOT_FRAME_Y: f32 = 10.0;
pub const SPOT_ITEM_Y: f32 = 15.0;
/// Where a spotlight's text starts, under its title.
pub const SPOT_TEXT_Y: f32 = 40.0;

/// An image page's picture, an entity's box and the gap under it.
pub const PICTURE: f32 = 80.0;
pub const ENTITY_Y: f32 = 12.0;
pub const ENTITY_ROOM: f32 = PICTURE + GAP;
/// What an image's or an entity's page takes above its text: its title, the picture, the gaps.
pub const PICTURE_PAGE_ABOVE: f32 = 104.0;

/// A multiblock page's view of its structure, and its name's room.
pub const MB_VIEW: f32 = 70.0;
pub const MB_NAME_H: f32 = PICTURE_TITLE_ROOM;

/// Room a recipe layout gets under itself.
pub const RECIPE_ROOM: f32 = 6.0;
/// A crafting grid's height: what a recipe page's text sits under (a furnace's is less).
pub const RECIPE_H: f32 = 80.0;
/// A relations page: its header, and each entry's line.
pub const RELATIONS_HEAD: f32 = 12.0;
pub const RELATIONS_LINE: f32 = 10.0;
/// An item element (a mod's page type): its icon and the gap under it.
pub const ITEM_ROOM: f32 = 20.0;

/// The room kept between a page's body and its navigation.
pub const BODY_PAD: f32 = 14.0;
/// Width of a page's text, and the height its body has for text: the page
/// less its title and separator, the navigation and the padding.
pub const TEXT_W: f32 = PAGE_W - TEXT_INSET;
pub const BODY_H: f32 = PAGE_H - BODY_Y - NAV_H - BODY_PAD;

/// Blocks shown at a slot's angle (a station), as the game shows a block
/// item; a multiblock turns slowly about the vertical, swaying this far.
pub const SLOT_YAW: f32 = 225.0;
pub const MULTIBLOCK_YAW: f32 = -45.0;
pub const MULTIBLOCK_SWAY: f32 = 20.0;
/// How far blocks are tilted toward the viewer.
pub const BLOCK_PITCH: f32 = 30.0;
/// An entity turns this many degrees a second, from this angle.
pub const ENTITY_TURN: f32 = 30.0;
pub const ENTITY_YAW: f32 = -30.0;
