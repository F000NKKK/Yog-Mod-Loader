//! A hand-written View in the shape `.yui` codegen produces: a search box
//! (two-way), filter toggles, a selectable virtualized list with an empty
//! state, a details presenter and a close button.

use yog_api::ui::event::keys;
use yog_api::ui::{Action, FlexDir, StateFlags, StylePatch, Units, Widget, widget};
use yog_kit::bind::{self, Slot};
use yog_kit::items::ItemsHost;
use yog_kit::testing::TestHost;
use yog_kit::{Anchor, Cx, Parts, ScreenMeta, Update, ViewEvent, ViewModel};

#[derive(Default)]
struct Vm {
    items: Vec<(String, bool)>,
    search: String,
    only_even: bool,
    selected: Option<String>,
    opened: u32,
}

impl ViewModel for Vm {
    fn on_open(&mut self, _cx: &mut Cx<'_, Self>) {
        self.opened += 1;
        if self.items.is_empty() {
            self.items = (0..100)
                .map(|i| (format!("item {i}"), i % 2 == 0))
                .collect();
        }
    }
}

impl Vm {
    fn visible(&self) -> impl Iterator<Item = &(String, bool)> + '_ {
        self.items
            .iter()
            .filter(move |(n, even)| (!self.only_even || *even) && n.contains(self.search.as_str()))
    }
    fn count_text(&self) -> String {
        format!("{} shown", self.visible().count())
    }
    fn selected(&self) -> Option<&(String, bool)> {
        match &self.selected {
            Some(s) => self.visible().find(|(n, _)| n == s),
            None => self.visible().next(),
        }
    }
    fn selected_id(&self) -> Option<&str> {
        self.selected().map(|(n, _)| n.as_str())
    }
    fn select(&mut self, id: String) {
        self.selected = Some(id);
    }
    fn toggle_even(&mut self) {
        self.only_even = !self.only_even;
    }
}

struct T0 {
    b0: Slot<String>,
}

impl T0 {
    fn build() -> (Self, Widget) {
        let w = widget::panel(FlexDir::Column)
            .w(Units::Percent(100.0))
            .h(Units::Px(10.0))
            .state_style(
                StateFlags::SELECTED,
                StylePatch {
                    bg: Some(yog_api::ui::Color(0xFF333366)),
                    ..Default::default()
                },
            )
            .child(widget::label("").id("row_label"));
        (T0 { b0: Slot::new() }, w)
    }
}

struct T1 {
    b0: Slot<String>,
}

impl T1 {
    fn build() -> (Self, Widget) {
        (T1 { b0: Slot::new() }, widget::label("").id("details"))
    }
}

fn empty0() -> Widget {
    widget::label("").id("empty")
}

struct View {
    b0: Slot<String>,
    b1: Slot<String>,
    b2: Slot<bool>,
    b3: Slot<u64>,
    l0: ItemsHost<T0>,
    l1: ItemsHost<T1>,
}

// content: Column
//   [0] Row: [0] Label count, [1] Button close
//   [1] TextBox search
//   [2] ToggleButton even
//   [3] ListView (scroll_view, items)
//   [4] ContentPresenter
impl yog_kit::View for View {
    type Vm = Vm;
    const META: ScreenMeta = ScreenMeta {
        id: "test:list",
        w: 200.0,
        h: 120.0,
        anchor: Anchor::Center,
        modal: true,
        pause: false,
        live: false,
    };

    fn build() -> (Self, Widget, Parts) {
        let root = widget::panel(FlexDir::Column)
            .child(
                widget::panel(FlexDir::Row)
                    .h(Units::Px(12.0))
                    .child(widget::label("").id("count"))
                    .child(
                        widget::button("Close")
                            .id("close")
                            .w(30.0)
                            .h(10.0)
                            .action(Action::new(0)),
                    ),
            )
            .child(
                widget::text_box("")
                    .id("search")
                    .w(Units::Percent(100.0))
                    .h(Units::Px(12.0)),
            )
            .child(
                widget::button("Even")
                    .id("even")
                    .w(30.0)
                    .h(10.0)
                    .action(Action::new(1)),
            )
            .child(
                widget::scroll_view()
                    .id("list")
                    .h(Units::Px(50.0))
                    .w(Units::Percent(100.0)),
            )
            .child(widget::panel(FlexDir::Column).id("presenter"));
        let mut parts = Parts::new();
        parts.focusable(&[0, 1]);
        parts.text_box(&[1]);
        parts.focusable(&[2]);
        let s0 = parts.scroller(&[3]);
        parts.list(1, &[3], Some(s0), Some(10.0));
        parts.list(2, &[4], None, None);
        let v = View {
            b0: Slot::new(),
            b1: Slot::new(),
            b2: Slot::new(),
            b3: Slot::new(),
            l0: ItemsHost::new(1, &[3], Some(10.0), Some(s0), true),
            l1: ItemsHost::new(2, &[4], None, None, false),
        };
        (v, root, parts)
    }

    fn update(&mut self, root: &mut Widget, vm: &Vm, u: &mut Update) {
        if self.b0.set(bind::text(&vm.count_text())) {
            root.at_path_mut(&[0, 0]).set_text(self.b0.get().clone());
            u.relayout();
        }
        if self.b1.set(bind::text(&vm.search)) {
            u.set_text_box(0, self.b1.get());
        }
        if self.b2.set(bind::flag(vm.only_even)) {
            bind::set_checked(root.at_path_mut(&[2]), *self.b2.get());
            u.repaint();
        }
        if self.b3.set(bind::key(&(vm.only_even, vm.search.as_str()))) {
            u.reset_scroll(0);
        }
        {
            let sel = vm.selected_id();
            let mut it = self.l0.begin(root, u);
            for item in vm.visible() {
                let selected = bind::is_selected(&sel, &item.0);
                let Some(row) = it.next(bind::key(&item.0), selected, T0::build) else {
                    continue;
                };
                if row.inst.b0.set(bind::text(&item.0)) {
                    row.w.at_path_mut(&[0]).set_text(row.inst.b0.get().clone());
                    row.u.relayout();
                }
            }
            if let Some((w, u)) = it.finish(Some(empty0)) {
                let _ = u;
                w.set_text("nothing");
            }
        }
        {
            let mut it = self.l1.begin(root, u);
            for item in ::std::iter::IntoIterator::into_iter(vm.selected()) {
                let Some(row) = it.next(bind::key(&item.0), false, T1::build) else {
                    continue;
                };
                if row.inst.b0.set(bind::text(&item.0)) {
                    row.w.set_text(row.inst.b0.get().clone());
                    row.u.relayout();
                }
            }
            it.finish(None);
        }
    }

    fn dispatch(&mut self, vm: &mut Vm, cx: &mut Cx<'_, Vm>, ev: ViewEvent) {
        match ev {
            ViewEvent::Action(Action {
                owner: 0, slot: 0, ..
            }) => cx.close(),
            ViewEvent::Action(Action {
                owner: 0, slot: 1, ..
            }) => vm.toggle_even(),
            ViewEvent::TextChanged { slot: 0, text } => vm.search = text,
            ViewEvent::Action(Action {
                owner: 1,
                slot: yog_kit::items::SELECT,
                index,
            })
            | ViewEvent::Select { list: 1, index } => {
                let k = vm.visible().nth(index as usize).map(|item| item.0.clone());
                if let Some(k) = k {
                    vm.select(k);
                }
            }
            _ => {}
        }
    }
}

fn host() -> TestHost<View> {
    let mut h = TestHost::<View>::new(Vm::default());
    h.open();
    h
}

#[test]
fn opens_and_binds() {
    let h = host();
    assert_eq!(h.vm().opened, 1);
    assert_eq!(h.text("count").as_deref(), Some("100 shown"));
    assert_eq!(h.text("details").as_deref(), Some("item 0"));
    // virtualized: 50px view, 10px rows → only the first rows exist
    let list = h.widget("list").unwrap();
    assert!(list.children.len() < 20, "{} children", list.children.len());
    assert!(list.children[1].state.contains(StateFlags::SELECTED));
}

#[test]
fn commands_and_two_way_text() {
    let mut h = host();
    h.click_id("even");
    assert!(h.vm().only_even);
    assert!(h.state("even").contains(StateFlags::CHECKED));
    assert_eq!(h.text("count").as_deref(), Some("50 shown"));

    h.type_into("search", "8");
    assert_eq!(h.vm().search, "8");
    assert_eq!(h.text("search").as_deref(), Some("8"));
    // even items containing "8": 8, 18, 28, 38, 48, 58, 68, 78, 80..88 even (5), 98
    assert_eq!(h.text("count").as_deref(), Some("14 shown"));

    // Esc leaves the box first, then is not consumed (the host closes)
    assert!(h.key(keys::ESCAPE));
    assert!(!h.key(keys::ESCAPE));

    h.click_id("close");
    assert!(h.closed());
}

#[test]
fn list_click_keys_and_reveal() {
    let mut h = host();
    // click the second row
    let list = h.path("list");
    let mut row = list.clone();
    row.push(2); // [spacer, row0, row1, ...]
    h.click_path(&row);
    assert_eq!(h.vm().selected.as_deref(), Some("item 1"));
    assert_eq!(h.text("details").as_deref(), Some("item 1"));

    // the list has focus now: Down moves the selection
    h.key(keys::DOWN);
    assert_eq!(h.vm().selected.as_deref(), Some("item 2"));

    // End selects the last item and scrolls it into view (virtualized)
    h.key(keys::END);
    assert_eq!(h.vm().selected.as_deref(), Some("item 99"));
    assert!(h.scroll_offset(0) > 900.0, "offset {}", h.scroll_offset(0));
    let list = h.widget("list").unwrap();
    let last_row = &list.children[list.children.len() - 2];
    assert_eq!(last_row.action.unwrap().index, 99);
    assert!(last_row.state.contains(StateFlags::SELECTED));
}

#[test]
fn empty_state_and_scroll_reset() {
    let mut h = host();
    h.key(keys::END);
    assert!(h.scroll_offset(0) > 0.0);
    h.type_into("search", "zzz");
    assert_eq!(h.scroll_offset(0), 0.0, "scroll_key changed");
    assert!(h.is_visible("empty"));
    assert_eq!(h.text("empty").as_deref(), Some("nothing"));
    assert!(h.widget("details").is_none());
}

#[test]
fn tab_cycles_focus_and_enter_activates() {
    let mut h = host();
    h.key(keys::TAB); // close button
    assert!(h.state("close").contains(StateFlags::FOCUSED));
    h.key(keys::TAB); // search box
    h.type_text("item 4");
    assert_eq!(h.vm().search, "item 4");
    h.key(keys::TAB); // even toggle
    h.key(keys::ENTER);
    assert!(h.vm().only_even);
    h.key_mods(keys::TAB, keys::MOD_SHIFT); // back to search
    h.key(keys::BACKSPACE);
    assert_eq!(h.vm().search, "item ");
}

#[test]
fn handle_updates_and_hover() {
    let mut h = host();
    let handle = h.handle();
    std::thread::spawn(move || handle.update(|vm| vm.only_even = true))
        .join()
        .unwrap();
    h.frame();
    assert_eq!(h.text("count").as_deref(), Some("50 shown"));
    let r = h.rect("even").unwrap();
    h.move_to(r.x + 1.0, r.y + 1.0);
    assert!(h.state("even").contains(StateFlags::HOVER));
}
