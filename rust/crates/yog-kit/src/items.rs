//! Lists: an [`ItemsHost`] keeps one template instance per item of an
//! `ItemsControl` / `ListView` / `ContentPresenter`, reconciled by key, so
//! an update only patches what changed. With a fixed `item_height` only the
//! rows in (and near) view are built — the rest are two spacers.
//!
//! Generated code drives it like this:
//!
//! ```ignore
//! if let Some(mut it) = self.l0.begin(root, u) {
//!     for (index, item) in vm.visible().enumerate() {
//!         let selected = bind::is_selected(&sel, &item.id);
//!         let Some(row) = it.next(bind::key(&item.id), selected, __T0::build) else { continue };
//!         if let Some(v) = row.inst.b0.changed(bind::text(&item.name)) {
//!             if let Some(w) = row.w.get_path_mut(&[0]) { w.set_text(v.clone()); row.u.relayout(); }
//!         }
//!     }
//!     if let Some((w, u)) = it.finish(Some(__E0)) { /* empty-state bindings */ }
//! }
//! ```

use std::collections::HashMap;

use yog_api::ui::{Action, StateFlags, Units, Widget, widget};

use crate::Update;

/// `Action::slot` of an item container: clicking it selects the item.
pub const SELECT: u16 = Action::SELECT;

/// Rows built beyond each edge of the visible window.
const OVERSCAN: u32 = 2;
/// Rows built before the list's height is known.
const FIRST_WINDOW: u32 = 32;

/// The instances of one list.
pub struct ItemsHost<T> {
    owner: u16,
    path: &'static [u16],
    item_height: Option<f32>,
    scroller: Option<u16>,
    selectable: bool,
    /// Keys and instances of the built items, in child order.
    insts: Vec<(u64, T)>,
    /// The empty-state widget while it isn't shown.
    empty: Option<Widget>,
    showing_empty: bool,
}

impl<T> ItemsHost<T> {
    /// The list `owner` whose items container is at `path`; `item_height`
    /// virtualizes it (rows are that tall), `scroller` is the index of its
    /// scroll view in the view's parts; `selectable` gives item containers
    /// the [`SELECT`] action.
    pub fn new(
        owner: u16,
        path: &'static [u16],
        item_height: Option<f32>,
        scroller: Option<u16>,
        selectable: bool,
    ) -> Self {
        Self {
            owner,
            path,
            item_height,
            scroller,
            selectable,
            insts: Vec::new(),
            empty: None,
            showing_empty: false,
        }
    }

    /// Starts an update of the items under `root` (the tree `path` is
    /// relative to); none when `root` has no widget there.
    pub fn begin<'a>(
        &'a mut self,
        root: &'a mut Widget,
        u: &'a mut Update,
    ) -> Option<ItemsCursor<'a, T>> {
        let container = root.get_path_mut(self.path)?;
        let mut old_children = std::mem::take(&mut container.children);
        if self.showing_empty {
            self.empty = old_children.pop();
            self.showing_empty = false;
        }
        let window = self.item_height.map(|ih| {
            let stride = ih + container.style.gap;
            let vp = self.scroller.map(|s| u.viewport(s)).unwrap_or_default();
            if vp.height <= 0.0 || stride <= 0.0 {
                return (0, FIRST_WINDOW);
            }
            let top = (vp.offset - container.style.pad[0]).max(0.0);
            let first = (top / stride) as u32;
            let rows = (vp.height / stride).ceil() as u32 + 1;
            (first.saturating_sub(OVERSCAN), first + rows + OVERSCAN)
        });
        if window.is_some() {
            // [top spacer, items..., bottom spacer]
            if !old_children.is_empty() {
                old_children.remove(0);
                old_children.pop();
            }
        }
        let mut old = HashMap::with_capacity(self.insts.len());
        for ((key, inst), w) in self.insts.drain(..).zip(old_children) {
            old.entry(key).or_insert((inst, w));
        }
        Some(ItemsCursor {
            owner: self.owner,
            selectable: self.selectable,
            item_height: self.item_height,
            window,
            host_insts: &mut self.insts,
            host_empty: &mut self.empty,
            showing_empty: &mut self.showing_empty,
            container,
            u,
            old,
            widgets: Vec::new(),
            index: 0,
            first_built: None,
            last_built: 0,
            selected: None,
        })
    }
}

/// One item handed to the template's bindings.
pub struct Item<'c, T> {
    pub inst: &'c mut T,
    pub w: &'c mut Widget,
    pub u: &'c mut Update,
}

/// An update of an [`ItemsHost`] in progress.
pub struct ItemsCursor<'a, T> {
    owner: u16,
    selectable: bool,
    item_height: Option<f32>,
    window: Option<(u32, u32)>,
    host_insts: &'a mut Vec<(u64, T)>,
    host_empty: &'a mut Option<Widget>,
    showing_empty: &'a mut bool,
    container: &'a mut Widget,
    u: &'a mut Update,
    old: HashMap<u64, (T, Widget)>,
    widgets: Vec<Widget>,
    index: u32,
    first_built: Option<u32>,
    last_built: u32,
    selected: Option<u32>,
}

/// Points every action of this list in `w`'s subtree at item `index`.
fn stamp(w: &mut Widget, owner: u16, index: u32) {
    if let Some(a) = &mut w.action
        && a.owner == owner
    {
        a.index = index;
    }
    for c in &mut w.children {
        stamp(c, owner, index);
    }
}

impl<'a, T> ItemsCursor<'a, T> {
    /// The next item, keyed `key` (see [`crate::bind::key`]); `build`
    /// makes a new instance. `None` when it's outside the visible window
    /// (virtualized lists) — skip its bindings.
    pub fn next(
        &mut self,
        key: u64,
        selected: bool,
        build: impl FnOnce() -> (T, Widget),
    ) -> Option<Item<'_, T>> {
        let index = self.index;
        self.index += 1;
        if selected {
            self.selected = Some(index);
        }
        if let Some((first, last)) = self.window
            && (index < first || index >= last)
        {
            return None;
        }
        let (inst, mut w) = match self.old.remove(&key) {
            Some(x) => x,
            None => {
                let (inst, mut w) = build();
                if self.selectable && w.action.is_none() {
                    w.action = Some(Action {
                        owner: self.owner,
                        slot: SELECT,
                        index,
                    });
                }
                self.u.relayout();
                (inst, w)
            }
        };
        stamp(&mut w, self.owner, index);
        if w.state.contains(StateFlags::SELECTED) != selected {
            w.set_state_inherited(StateFlags::SELECTED, selected);
            self.u.repaint();
        }
        self.first_built.get_or_insert(index);
        self.last_built = index + 1;
        let at = self.widgets.len();
        self.host_insts.push((key, inst));
        self.widgets.push(w);
        let (_, inst) = &mut self.host_insts[at];
        Some(Item {
            inst,
            w: &mut self.widgets[at],
            u: self.u,
        })
    }

    /// Ends the update. With `empty` (the `.Empty` template's builder) and
    /// no items, shows that widget and returns it for its bindings.
    pub fn finish(self, empty: Option<fn() -> Widget>) -> Option<(&'a mut Widget, &'a mut Update)> {
        let n = self.index;
        if !self.old.is_empty() {
            self.u.relayout();
        }
        self.u.list_info(self.owner, n, self.selected);
        let mut children = self.widgets;
        if let (Some(ih), Some(_)) = (self.item_height, self.window) {
            let stride = ih + self.container.style.gap;
            let gap = self.container.style.gap;
            let first = self
                .first_built
                .unwrap_or(n.min(self.window.map_or(0, |w| w.0)));
            let after = n.saturating_sub(self.last_built.max(first));
            let spacer = |rows: u32| {
                widget::spacer()
                    .w(Units::Px(0.0))
                    .h(Units::Px((rows as f32 * stride - gap).max(0.0)))
                    .visible(rows > 0)
            };
            children.insert(0, spacer(first));
            children.push(spacer(after));
        }
        if n == 0
            && let Some(build) = empty
        {
            let w = self.host_empty.take().unwrap_or_else(build);
            children.push(w);
            *self.showing_empty = true;
            let at = children.len() - 1;
            self.container.children = children;
            return Some((&mut self.container.children[at], self.u));
        }
        self.container.children = children;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Viewport;
    use yog_api::ui::FlexDir;

    struct Row;

    fn row() -> (Row, Widget) {
        (Row, widget::label(""))
    }

    fn root() -> Widget {
        widget::panel(FlexDir::Column).child(widget::scroll_view().gap(2.0))
    }

    fn run(
        host: &mut ItemsHost<Row>,
        root: &mut Widget,
        u: &mut Update,
        keys: &[u64],
        sel: u64,
    ) -> u32 {
        let mut built = 0;
        let Some(mut it) = host.begin(root, u) else {
            return 0;
        };
        for &k in keys {
            if let Some(item) = it.next(k, k == sel, row) {
                let Row = item.inst;
                item.w.set_text(k.to_string());
                built += 1;
            }
        }
        it.finish(Some(|| widget::label("empty")));
        built
    }

    #[test]
    fn reconciles_by_key_and_stamps_indices() {
        let mut host = ItemsHost::new(1, &[0], None, None, true);
        let mut r = root();
        let mut u = Update::new();
        run(&mut host, &mut r, &mut u, &[10, 20, 30], 20);
        let list = &r.children[0];
        assert_eq!(list.children.len(), 3);
        assert_eq!(list.children[1].action.unwrap().index, 1);
        assert!(list.children[1].state.contains(StateFlags::SELECTED));
        assert_eq!(u.lists, vec![(1, 3, Some(1))]);

        let mut u = Update::new();
        run(&mut host, &mut r, &mut u, &[30, 10], 0);
        let list = &r.children[0];
        assert_eq!(list.children.len(), 2);
        assert_eq!(list.children[0].action.unwrap().index, 0);
        assert!(!list.children[1].state.contains(StateFlags::SELECTED));
        assert!(u.needs_relayout(), "an item was removed");

        let mut u = Update::new();
        run(&mut host, &mut r, &mut u, &[], 0);
        let list = &r.children[0];
        assert_eq!(list.children.len(), 1, "empty state shown");
        let mut u = Update::new();
        run(&mut host, &mut r, &mut u, &[5], 0);
        assert_eq!(r.children[0].children.len(), 1);
        assert!(host.empty.is_some(), "empty widget kept for later");
    }

    #[test]
    fn virtualizes_with_item_height() {
        let mut host = ItemsHost::new(1, &[0], Some(10.0), Some(0), true);
        let mut r = root();
        let keys: Vec<u64> = (0..1000).collect();
        let mut u = Update {
            viewports: vec![Viewport {
                offset: 1200.0,
                height: 60.0,
            }],
            ..Update::new()
        };
        let built = run(&mut host, &mut r, &mut u, &keys, 0);
        // stride 12: first visible row 100, 5 rows + 1, overscan 2 each side
        assert_eq!(built, 10);
        let list = &r.children[0];
        assert_eq!(list.children.len(), built as usize + 2);
        let top = &list.children[0];
        assert_eq!(top.style.h, Units::Px(98.0 * 12.0 - 2.0));
        assert_eq!(list.children[1].action.unwrap().index, 98);
        let bottom = list.children.last().unwrap();
        assert_eq!(bottom.style.h, Units::Px((1000.0 - 108.0) * 12.0 - 2.0));
        assert_eq!(u.lists, vec![(1, 1000, Some(0))]);
    }
}
