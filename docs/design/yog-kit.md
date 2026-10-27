# Plan: MVVM framework (yog-kit), typed resources (.yres) and scaffolding for Yog mod UI

This is a read-only design. Nothing was modified. It assumes the in-flight work lands first: TextBox, fonts/shadow, close_ui, `.ylang` + `@key`, Image aliases (`item:`/`sprite:`), and `.g.rs` generation into `obj/generated` with `yog_api::ui_include!`.

---

## 0. Summary and changes to the proposed direction

**What I keep from the proposal:**
- WinUI `x:Bind`-style compiled bindings, generated into the screen's `.g.rs`, with paths checked by rustc.
- ListView / ItemsControl with DataTemplate, SelectedItem, ToggleButton/IsChecked, Styles and Resources, visual states.
- Built-in scrolling, focus and keyboard handling.
- Navigation and dialogs.
- Design-time data.
- A new framework crate, `yog-kit`.

**What I change, and why:**

1. **No `INotifyPropertyChanged`, and no proc-macros in v1.**
   - Each binding site stores the last value it wrote in a typed `Slot<T: PartialEq>`.
   - After any `&mut` access to the VM (a command, a two-way write-back, `ScreenHandle::update`, a lifecycle hook), the host re-evaluates every binding once. Only changed widgets are patched. Layout reruns only if a layout-affecting value changed. Frames that change nothing only render the cached tree.
   - With fewer than ~200 bindings per screen this is cheaper than dirty-flag bookkeeping. It can't be broken by forgetting `notify`, and the VM stays a plain Rust struct.
   - `#[observable]`, `#[computed]` and `#[command]` would duplicate what the codegen already knows. Commands are just methods called by generated `match` arms. The `yog-kit-macros` name is reserved for later sugar (§14, P9).
2. **Converters are plain functions.** `{x:Bind source_color(source)}` or `{x:Bind Strings::count(n)}`. There is no `IValueConverter`. Function bindings are compile-checked, including arity.
3. **Visual states are attribute prefixes, not VisualStateManager XML.** For example `hover:bg="…"`, `selected:color="…"`, `checked:bg="…"`. They are restricted to paint-only properties, so a state change never relayouts. Styles use the same prefixes.
4. **Events are typed, with no string ids.**
   - Widgets carry `action: Option<Action>`, a `Copy` struct of `{owner, slot, index}`, set by codegen.
   - Hit-testing returns the action, and generated `dispatch` matches on it.
   - Raw host strings are parsed exactly once, in `yog_ui::UiEvent::parse`.
5. **Templates need no item type name.**
   - Template bindings are emitted inline inside a `for item in <items expr>` loop, so rustc infers the type.
   - Item commands re-resolve the item by index at dispatch time. Captured arguments are cloned before the `&mut vm` call.
   - `x:DataType` is optional and only used by the IDE.
6. **Resources replace `x:Tr`.**
   - `@res:Strings.title` in markup and `Strings::count(n)` in bindings are typed accessors generated from `resources/*.yres`.
   - The `x:Tr` markup extension is unnecessary.
7. **One-line registration comes from the generated `.g.rs`** (`pub fn register`), not from a `#[yog::screen]` macro.
8. **ABI impact: none on the `YogApi` table.**
   - Three new event strings travel over the existing `register_ui` callback: `move:X:Y`, `open:W:H`, `close`. They need runtime/Java changes only.
   - Hover and wheel-routing need `move`. Without it they degrade gracefully.

---

## 1. Architecture

```
yog-abi ── yog-gfx ── yog-ui (widget tree, layout, render; + Action, visible, StateFlags/StylePatch,
   │                    │       UiEvent parser, hit_path/at_mut, ScrollState/TextBoxState unchanged)
   └──────────── yog-api (Registry, tr, open_ui/close_ui, ui_include!, resources!, tr/tr_args)
                          │
                       yog-kit (NEW: ViewModel, View trait, Screen<V> host, Slot/Update, Cx, ScreenHandle,
                                ItemsHost (lists/templates/virtualization), focus/keyboard, scroll routing,
                                overlays/dialogs, Navigator, testing::TestHost)
yog-cli (codegen, no runtime dep): src/yui/{xml,model,binding,resources,gen_legacy,gen_vm,sourcemap,schema}.rs,
         src/yres.rs (resources → generated/resources/<Set>.g.rs + lang JSON + assets; already implemented), src/templates.rs (+ templates/ dir),
         cargo diagnostic remapping
Yog-IDLE: designer (TS + src-tauri/src/designer/markup.rs), binding editor, sample data, .yres editor,
          New Project / New Item from `yog templates`
Java hosts + yog-runtime/ui_jni.rs: emit move/open/close event strings
```

**Where each piece lives:**

| Concern | Location | Why |
|---|---|---|
| Action ids, visibility, state flags, style patches, path access, event parsing | yog-ui | Pure widget concerns. Usable without kit (book, inventory). |
| `.yres` support: `resources!()`, `tr`/`tr_args` | yog-api | Every mod uses resources: metadata, items, UI. |
| Screen hosting, binding runtime, lists, focus, dialogs, navigation | yog-kit (depends on yog-api) | Opt-in framework. Versioned separately from yog-api. |
| `.yui` → View code, `.yres` → `resources/<Set>.g.rs`, templates, diagnostics | yog-cli | Compile-time only. |
| Proc-macros | none in v1 (`yog-kit-macros` reserved) | Codegen covers it. |

**Other placement decisions:**
- yog-api cannot re-export yog-kit because that would be a cycle. Mods depend on `yog-kit` directly. yog-cli auto-adds `yog-kit = "<compatible>"` to `obj/Cargo.toml` when any `.yui` has `x:ViewModel` and `yog.toml` doesn't list it. With a local `yog_api_path` it gets the same local path patch as the other yog-* crates (main.rs ~940, `local_api_patch`).
- Version guard: generated code starts with `const _: () = ::yog_kit::__codegen::require(1);`. This is a const fn that panics with "yog-kit X expects codegen N; update yog". A newer yog-kit keeps accepting older codegen versions it still supports.
- Each mod statically links its own yog-kit copy. Navigation and handles are intra-mod. Cross-mod navigation uses UI ids (`open_ui`).

---

## 2. Runtime model

**State ownership.**
- `yog_kit::Screen<W: View>` is the host. It owns `vm: W::Vm`, `view: W`, the laid-out `UiRoot`, the focus/hover/pressed path, the overlay stack and an update queue.
- It is created by the generated `register()`, stored in a `OnceLock<ScreenHandle<Vm>>` inside the `.g.rs`, and captured by the `on_ui_render` / `register_ui` closures. There is no global Mutex in user code.
- `V: Send + 'static`.

**Retained tree, patched in place. Not rebuilt.**
- `View::build()` creates the content tree once, on registration or first render.
- Each bound widget is addressed by a static child-index path (`&[u16]`), known at codegen time. Paths are stable because:
  - an ItemsControl owns its own child vector, so its siblings never shift;
  - invisible widgets stay in the tree as `visible=false`. Layout emits a zero-size, non-hit node for them, so the `LayoutNode` tree mirrors the `Widget` tree 1:1.
- Template instances live as children of their ItemsControl, reconciled by key.

**Change propagation.**

| Trigger | Effect |
|---|---|
| Command dispatch, two-way write-back, BindBack, `on_open`/`on_close`, queued `ScreenHandle::update`, dialog result | `dirty = true` |
| Scroll (a virtualized list's window may move) | that list is marked dirty |
| Hover/pressed/focus change | `repaint` only (state flags are paint-only) |
| Screen size change | relayout |

`render(gfx)` each frame:
1. Drain the update queue.
2. If dirty, run `view.update(&vm, &mut Update)`. It evaluates bindings, `Slot::set` compares, changed widgets are patched, and `Update` collects `relayout`.
3. If relayout is needed or the size changed: wrap in the anchored screen panel and run `UiRoot::layout`.
4. Apply per-frame TextBox caret blink (`TextBoxState::apply`, paint-only).
5. Set state flags.
6. Render the root, then overlays.

Events are always hit-tested against the layout of the last rendered frame, which is what the user saw. That matches today's `with_last` semantics.

**Event routing** happens in `Screen::on_event(&str) -> bool`, the consumed flag for `UiHandled`:
```
UiEvent::parse(raw) →
  Open{w,h}    → vm.on_open(cx); dirty
  Close        → vm.on_close(cx); clear focus/hover
  Move{x,y}    → hover = hit_path(layout, x, y); repaint if changed
  Scroll{dy}   → overlay? → scroller under the hover point (else focused list, else first ScrollView): ScrollState
  Click{x,y}   → overlay first → scrollbar track of any scroller (ScrollState::handle) →
                 TextBox focus/caret (TextBoxState) → hit_path → action → pressed=path →
                 view.dispatch(vm, cx, ViewEvent::Action(a)) → dirty
  Drag/Release → knob drag / textbox selection / clear pressed
  Key{k,mods}  → focused TextBox (Changed → ViewEvent::TextChanged{slot} → write-back; Submitted)
                 else Tab/Shift+Tab focus cycle, Enter/Space activate focused, Up/Down/PgUp/PgDn/Home/End in
                 focused ListView (BindBack with neighbour key, scroll into view), Esc → vm.on_back(cx) → default
                 cx.back() (pop navigation or close)
  Char{c}      → focused TextBox
```

**Lists:**
- `ItemsHost` reconciles by key: `u64` hash of the `key` binding, or the index when no key is given.
- Instances are reused, created by the template's `build()`, removed, or reordered by moving `Widget`s out of the old child vector.
- ListView's item container carries `Action{owner: list, slot: SELECT, index}`. Deeper actions, such as a button inside a row, win the hit test.
- **Virtualization:** when `item_height` is set, the host computes the visible window from `ScrollState.offset` and the viewport height of the last layout, plus 2 rows of overscan.
  - The children are `[top Spacer(h=first*stride), items[first..last], bottom Spacer]`.
  - `ItemsCursor::next` returns `None` for indices outside the window, so their bindings aren't evaluated.
  - Without `item_height`, all items are built.

**Thread safety.**
- Everything runs on the render thread. `ScreenHandle::update` from any thread pushes a boxed `FnOnce(&mut V) + Send` onto a separate queue Mutex. It never takes the host lock, so calling it from inside a command can't deadlock.
- `ScreenHandle::read` uses `try_lock`.

**Localization.** `@res:Strings.x` text in VM screens is compiled as a binding, re-evaluated on every update, including `open`. A language switch shows up the next time the screen opens or updates.

---

## 3. yog-ui changes (additive; bump 0.36 → 0.37)

`widget.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Action { pub owner: u16, pub slot: u16, pub index: u32 }   // owner 0 = screen, k = ItemsControl k

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateFlags(pub u8);
impl StateFlags { pub const HOVER: Self; pub const PRESSED: Self; pub const FOCUSED: Self;
                  pub const SELECTED: Self; pub const CHECKED: Self; pub const DISABLED: Self;
                  pub fn contains(self, o: Self) -> bool; pub fn insert(&mut self, o: Self); }

/// Paint-only overrides — never affect layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StylePatch { pub bg: Option<u32>, pub color: Option<u32>, pub shadow: Option<bool>,
                        pub focus_color: Option<u32>, pub border: Option<u32> }
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateStyles { pub rules: Vec<(StateFlags, StylePatch)> }   // later rules win

pub struct Widget { /* existing … */
    pub action: Option<Action>,
    pub visible: bool,                       // false = collapsed: no space, not drawn, not hit
    pub state: StateFlags,                   // set by the host each frame (inherited, see below)
    pub states: Option<Box<StateStyles>>,
}
impl Widget {
    pub fn action(self, a: Action) -> Self;
    pub fn visible(self, v: bool) -> Self;
    pub fn state_style(self, when: StateFlags, patch: StylePatch) -> Self;
    pub fn at_mut(&mut self, path: &[u16]) -> &mut Widget;      // panics on a bad path (codegen-only)
    pub fn at(&self, path: &[u16]) -> &Widget;
    /// Sets `flags` on self and descendants; stops at descendants with their own `action`.
    pub fn set_state_inherited(&mut self, mask: StateFlags, on: bool);
    pub fn effective_style(&self) -> std::borrow::Cow<'_, Style>;  // base + matching state patches
}
```
- **State inheritance:** HOVER, PRESSED, SELECTED and CHECKED set on an interactive widget, or on a list item container, flow down to its non-interactive descendants. A row's label can therefore react to `selected:` or `hover:` of its row. DISABLED is set wherever `enabled=false` and is inherited.
- `layout.rs`:
  - `LayoutNode` gains `action: Option<Action>` and `visible: bool`.
  - Invisible widgets produce a zero rect, skip their children, and aren't hit.
  - `hit_test` treats `action.is_some() || on_click.is_some()` as clickable.
  - New functions:
    - `pub fn hit_path(node: &LayoutNode, mx: f32, my: f32) -> Option<Vec<u16>>`
    - `pub fn node_at<'a>(node: &'a LayoutNode, path: &[u16]) -> Option<&'a LayoutNode>`
- `render.rs`: uses `effective_style()` and skips invisible widgets.
- New `event.rs`:
  ```rust
  pub enum UiEvent<'a> { Click{x:f32,y:f32}, Drag{x,y}, Release{x,y}, Scroll{dy:f32}, Move{x,y},
                         Key{key:i32,mods:i32}, Char(char), Open{w:f32,h:f32}, Close, Other(&'a str) }
  impl<'a> UiEvent<'a> { pub fn parse(raw: &'a str) -> Self; }
  ```
- `ScrollState::handle_event(&UiEvent, &LayoutNode, f32)` and `TextBoxState::handle_event(..)` are added. The string versions stay and delegate.
- `ToggleButton` needs no new kind. It is `WidgetKind::Button` plus the CHECKED flag, handled by kit and codegen.

---

## 4. yog-kit API (crate `rust/crates/yog-kit`, MIT/Apache)

```rust
pub mod prelude { pub use crate::{ViewModel, Cx, ScreenHandle, Dialog, DialogResult}; }

pub trait ViewModel: Send + 'static {
    fn on_open(&mut self, cx: &mut Cx<'_, Self>) where Self: Sized {}
    fn on_close(&mut self, cx: &mut Cx<'_, Self>) where Self: Sized {}
    /// Esc / back. `false` = default (pop navigation, else close).
    fn on_back(&mut self, cx: &mut Cx<'_, Self>) -> bool where Self: Sized { false }
    /// Escape hatch: imperative tweaks after each update (rarely needed).
    fn customize(&self, root: &mut yog_ui::Widget) {}
}

/// Implemented only by generated code.
pub trait View: Sized + Send + 'static {
    type Vm: ViewModel;
    const META: ScreenMeta;               // id, size, anchor, modal, pause
    fn build() -> Self;
    fn root(&self) -> &yog_ui::Widget;
    fn root_mut(&mut self) -> &mut yog_ui::Widget;
    fn parts(&mut self) -> &mut Parts;    // scrollers, text boxes, lists, focus order (paths)
    fn update(&mut self, vm: &Self::Vm, u: &mut Update);
    fn dispatch(&mut self, vm: &mut Self::Vm, cx: &mut Cx<'_, Self::Vm>, ev: ViewEvent);
}
pub struct ScreenMeta { pub id: &'static str, pub w: f32, pub h: f32, pub anchor: Anchor, pub modal: bool, pub pause: bool }
pub enum Anchor { Center, TopLeft, Top, TopRight, Left, Right, BottomLeft, Bottom, BottomRight, Fill }
pub enum ViewEvent { Action(yog_ui::Action), TextChanged { slot: u16 }, TextSubmitted { slot: u16 },
                     Select { list: u16, index: u32 } }

pub struct Screen<W: View> { /* vm, view, ui: Option<UiRoot>, size, dirty, input, overlays, queue */ }
impl<W: View> Screen<W> {
    pub fn register(registry: &mut yog_api::Registry, vm: W::Vm) -> ScreenHandle<W::Vm>;
    // internals used by register: fn render(&mut self, gfx: &GfxContext); fn on_event(&mut self, raw: &str) -> bool;
    pub fn frame(&mut self, screen_w: f32, screen_h: f32) -> &yog_ui::UiRoot;  // gfx-free (tests, designer)
}

#[derive(Clone)] pub struct ScreenHandle<V> { /* Arc<queue>, Arc<Mutex<host>> */ }
impl<V: ViewModel> ScreenHandle<V> {
    pub fn update(&self, f: impl FnOnce(&mut V) + Send + 'static);  // queued, any thread
    pub fn read<R>(&self, f: impl FnOnce(&V) -> R) -> Option<R>;   // try_lock
    pub fn open(&self); pub fn close(&self);
}

pub struct Cx<'a, V> { /* &mut host internals, no &mut V */ }
impl<V: ViewModel> Cx<'_, V> {
    pub fn close(&mut self);
    pub fn navigate(&mut self, ui_id: &str);                          // push self, open other
    pub fn navigate_to<W: View>(&mut self, init: impl FnOnce(&mut W::Vm) + Send + 'static);
    pub fn back(&mut self);
    pub fn dialog(&mut self, d: Dialog<V>);
    pub fn show<D: View>(&mut self, vm: D::Vm, then: impl FnOnce(&mut V, &mut Cx<'_, V>, D::Vm) + Send + 'static);
    pub fn focus(&mut self, id: &str);
    pub fn handle(&self) -> ScreenHandle<V>;
    pub fn invalidate(&mut self);
}

pub struct Dialog<V> { /* … */ }
impl<V: ViewModel> Dialog<V> {
    pub fn message(title: impl Into<String>, text: impl Into<String>) -> Self;
    pub fn confirm(title: impl Into<String>, text: impl Into<String>) -> Self;
    pub fn prompt(title: impl Into<String>, initial: impl Into<String>) -> Self;
    pub fn on_result(self, f: impl FnOnce(&mut V, &mut Cx<'_, V>, DialogResult) + Send + 'static) -> Self;
}
pub enum DialogResult { Ok, Cancel, Text(String) }

pub mod bind {   // used by generated code
    pub struct Slot<T>(Option<T>);
    impl<T: PartialEq> Slot<T> { pub const fn new() -> Self; pub fn set(&mut self, v: T) -> bool; pub fn get(&self) -> &T; }
    #[diagnostic::on_unimplemented(message = "`{Self}` can't be shown as text in a .yui binding",
        note = "bind a value that implements Display, add `Fallback=`, or call a method returning String")]
    pub trait BindText { fn bind_text(&self) -> String; }   impl<T: Display + ?Sized> BindText for T {}
    pub trait BindColor { fn bind_color(&self) -> u32; }      // u32, Color, res::Color
    pub fn text<T: BindText + ?Sized>(v: &T) -> String;
    pub fn color<T: BindColor>(v: T) -> u32;
    pub fn flag(v: impl Into<bool>) -> bool;
    pub fn units(v: impl Into<yog_ui::Units>) -> yog_ui::Units;
    pub fn count<T: TryInto<u32>>(v: T) -> u32;
    pub fn key<K: Hash + ?Sized>(k: &K) -> u64;
    pub fn is_selected<S: PartialEq<K>, K: ?Sized>(sel: &Option<S>, key: &K) -> bool;
    pub fn sync_text(state: &mut yog_ui::TextBoxState, v: &str) -> bool;   // set only if different
}
pub struct Update { /* relayout, repaint flags, viewport info for lists */ }
impl Update { pub fn relayout(&mut self); pub fn repaint(&mut self); }

pub mod items {
    pub const SELECT: u16 = u16::MAX;
    pub struct ItemsHost<T> { /* instances: Vec<(u64, T)>, keys, window, empty */ }
    pub struct ItemsCursor<'a, T>;
    impl<T> ItemsHost<T> { pub fn new(owner: u16, path: &'static [u16], item_height: Option<f32>) -> Self;
        pub fn begin<'a>(&'a mut self, root: &'a mut Widget, u: &'a mut Update) -> ItemsCursor<'a, T>; }
    impl<'a, T> ItemsCursor<'a, T> {
        pub fn next(&mut self, key: u64, build: impl FnOnce() -> (T, Widget)) -> Option<(&mut T, &mut Widget)>;
        pub fn set_selected(&mut self, on: bool);
        pub fn finish(self, empty: Option<impl FnOnce() -> Widget>);
    }
}
pub mod testing { pub struct TestHost<W: View>; /* new(vm), size(w,h), frame(), click_id(id), click_path(&[u16]),
    type_text(id, &str), key(code), scroll(dy), move_to(x,y), vm(), vm_mut(), text_at(id) -> String,
    is_visible(id), state(id) -> StateFlags */ }
pub mod __codegen { pub const fn require(v: u32); }
```
Built-in dialogs are written in `.yui` inside yog-kit (`src/dialogs/{message,confirm,prompt}.yui`). Their generated `.g.rs` is committed in the kit, which dogfoods the codegen. A CI test (`yog ui gen --check`) keeps them current.

---

## 5. `.yui` grammar additions

**Namespaces.** `x:` (compile directives) and `d:` (design-time, ignored by `yog build`) are accepted without xmlns declarations. The parser already accepts `:` and `.` in names (yui.rs:94).

**Screen-level:**
- `x:ViewModel="ModListVm"`: a type path resolved in the code-behind module. Switches to VM codegen.
- `x:Create="ModListVm::new()"`: optional constructor expression. The default is `Default::default()`.
- `<Dialog …>` root: same as Screen but hosted as an overlay. It has no `id`.

**Property elements:** `<Screen.Resources>`, `<ListView.ItemTemplate>`, `<ListView.Empty>`, `<ContentPresenter.Empty>`, `<DataTemplate x:DataType="ModEntry">`.

**Attribute value forms:**

| Form | Meaning |
|---|---|
| `literal` | As today. |
| `@res:Set.name` | Typed resource from `resources/Set.yres`: string, image, color, number or style. Compiled to the generated accessor (`Set::name()` from `resources!()`). |
| `@name` (no dot) | Screen-local resource (`<Screen.Resources>`). |
| `@lang:key` | Any translation key, as the lang files have it (`@lang:gui.done`). |
| `@@text` | Literal starting with `@`. |
| `{x:Bind Expr[, Mode=OneTime\|OneWay\|TwoWay][, BindBack=method][, Fallback=value]}` | Compiled binding. |
| `res:Set.name` | Runtime string form of an image alias (also usable from Rust). |

Resolution order for `@a.b`: (1) `a` is a resource set → typed; (2) otherwise a raw key. A name that is both is an error.

**Expression grammar inside x:Bind.** It is deliberately small, and `<`, `&&` and `||` are excluded because of XML escaping. Use a VM method instead.
```
Expr     := Unary (('==' | '!=') Unary)?
Unary    := '!'? Postfix
Postfix  := Primary ( ('.' | '?.') Ident Call? )*
Primary  := Ident Call? | TypePath Call? | Literal | '(' Expr ')'
TypePath := Ident ('::' Ident)+            // emitted verbatim: Filter::Yog, Strings::count
Call     := '(' (Expr (',' Expr)*)? ')'
Literal  := int | float | true | false | 'single-quoted string'
```

**Scopes.** A bare root identifier means `vm.<id>` in the screen scope and `item.<id>` in a template. These roots are reserved:
- `vm`: the screen VM, also inside templates.
- `item`, `index`: template only.
- `cx`: command expressions only.

**`?.`** generates `(|| Some(conv(&<a>?.b)))().unwrap_or_else(|| <Fallback>)`. Without `Fallback=` the fallback is `Default::default()` of the converted target (`""`, `false`, `0`).

**Bindable attributes, by target kind:**
- text: `text`, `placeholder`, `item`, `texture`
- color: `bg`, `color`, `focus_color`
- flag: `enabled`, `visible`, `checked`
- units: `w`, `h`
- number: `count`, `gap`, `font_scale`
- `items`, `content`, `key`, `selected`, `scroll_key`

Binding layout attributes (`w`, `h`, `gap`, `padding`) is allowed and marks relayout.

**Two-way bindings:**
- `TextBox text="{x:Bind search, Mode=TwoWay}"` writes back `vm.search = text.to_owned()` on change. It needs a place expression (field path). Otherwise use `BindBack=set_search`, which calls `vm.set_search(String)`.
- `ToggleButton checked=… Mode=TwoWay` writes `vm.flag = !vm.flag`.
- `ListView selected="{x:Bind selected_id(), BindBack=select}"`: a click or arrow key calls `vm.select(<clone of key>)`.

The codegen rejects TwoWay on non-place expressions, with the line number.

**Commands:**
- `command="{x:Bind set_filter(Filter::Yog)}"` and `command="{x:Bind cx.close()}"`. Arguments are evaluated at click time.
- In templates, `item.*` arguments are cloned (`Clone` required) before the `&mut vm` call.
- `enabled` is bound separately (there is no CanExecute).
- `on_click`/`on_change`/`on_submit` are compile errors in VM screens: use `command=` and `Mode=TwoWay`.

**New elements:**

| Element | Attributes |
|---|---|
| `ToggleButton` | Button attributes plus `checked`. |
| `ItemsControl` | Column; `items`, `key`, `orientation`, `<ItemsControl.ItemTemplate>`, `<ItemsControl.Empty>`. |
| `ListView` | ScrollView + ItemsControl + `selected`, `item_height` (virtualization), keyboard selection, `scroll_key`. |
| `ContentPresenter` | `content` (an `Option<&T>`; an ItemsControl over 0..1 items), inline `DataTemplate`, `<ContentPresenter.Empty>`. |
| `ScrollView` | Gains `scroll_key="{x:Bind expr}"`: when the hashed value changes, scroll resets to the top. |

`x:Name` is an alias of `id`, used for designer naming.

**State prefixes.** `hover:`, `pressed:`, `focused:`, `selected:`, `checked:` and `disabled:` work on any paint attribute (`bg`, `color`, `shadow`, `focus_color`, `border`). Prefixing a layout attribute is an error.

**Resources and styles:**
```xml
<Screen.Resources>
    <Color x:Key="Row">#44333333</Color>
    <Style x:Key="FilterButton" target="ToggleButton" based_on="@res:Theme.Button"
           w="58" h="14" bg="#FF333333" color="#FFFFFF" hover:bg="#FF444444" checked:bg="#FF555555"
           disabled:color="#FF777777"/>
</Screen.Resources>
<ToggleButton style="@FilterButton" …/>
```
Styles are inlined at codegen. Explicit attributes override the style. `based_on` chains.

**Design-time:**
- `d:text`, `d:visible` and similar give a literal per element.
- `d:items="6"` repeats a template.
- `d:Sample="mod_list.sample.json"` on the Screen points to a JSON object keyed by the binding's expression text, e.g. `"count_text()": "58 mods"`, or `"visible()": [ {...}, ... ]` whose item objects are keyed by template expressions.

---

## 6. Resources: `.yres` (resx-like, typed)

**Implemented separately; the authoritative spec is [`Yog-IDLE/docs/yres.md`](../../../Yog-IDLE/docs/yres.md).** This section only summarizes what yog-kit relies on.

**Files:** `resources/**/<Set>.yres` (neutral culture = `en_us`) plus culture variants `<Set>.<locale>.yres` (e.g. `Strings.ru_ru.yres`; texts only). `Set` is an identifier, unique in the mod, and becomes the accessor type name.

**Format:** one JSON object; entry name → value. A value is a string (a text) or an object `{ "value", "type", "comment", "key", "texture", "event", "subtitle" }`; `type` is `string` (default), `image`, `file`, `sound`, `color`, `number`, `bool`:
```json
{
  "mod_name": "Mod List",
  "screen.title": { "value": "Yog Mods", "comment": "Title of the mod list screen" },
  "mods_count": { "value": "{count} mods", "comment": "{count}: number of mods" },
  "logo": { "type": "image", "value": "images/logo.png" },
  "accent": { "type": "color", "value": "#FF8800" },
  "rows_per_page": { "type": "number", "value": 20 },
  "show_badges": { "type": "bool", "value": true }
}
```

**Placeholders:** `{name}` or `{0}`, `{1}` (one style per text); compiled to `%1$s`… in lang JSON. The accessor takes one `impl Display` per placeholder, in order of first appearance.

**Translation keys:** `key` if given, otherwise by set: `Items` → `item.<modid>.<name>`, `Blocks` → `block.<modid>.<name>`, `Entities` → `entity.<modid>.<name>`, any other set → `<modid>.<name>`. So item/block display names need no lang file and no `for=` attribute.

**Build:** texts → `assets/<modid>/lang/<locale>.json`; images → `assets/<modid>/textures/<path relative to resources/>.png` (or `texture`); sounds → `assets/<modid>/sounds/…` + generated `sounds.json`; files → `assets/<modid>/<path>`.

**Generated code:** one `obj/generated/resources/<Set>.g.rs` per set (`// <auto-generated>` banner; Yog-IDLE nests it read-only under the set's neutral `.yres`, like `Resources.Designer.cs`), plus `obj/generated/resources.g.rs` that `include!`s them all. A single `yog_api::resources!();` in `lib.rs` includes that as `pub mod res`, so every set is `crate::res::<Set>` with nothing to add per set. Each set is a `pub struct <Set>;` with, per entry: texts → `fn name(args…) -> String` (looked up each call via `yog_api::tr`/`tr_args`) and `const NAME_KEY: &str`; images → `const fn name() -> &'static str` (image source `<modid>:<texture>`); files/sounds → resource/event ids; colors → `const NAME: u32`; numbers → `i64`/`f64` consts; bools → `bool` consts. Docs follow `Resources.Designer.cs`: `/// Looks up a localized string similar to "<neutral text>".` (or "The image …", "The color …", …), then the entry name, its comment and `file:line`.

**yog.toml:** `[mod] name`/`description` may be `@res:Strings.x`, `icon` may be `@res:Icons.x`; the manifest gets the neutral texts plus `name_key`/`description_key`.

**What yog-kit adds on top:**
- In `.yui`, `@<Set>.<name>` (already supported for `text`/`placeholder`/`texture`) compiles to the generated accessor; in bindings, accessors are plain functions: `{x:Bind Strings::mods_count(n)}`.
- Screen-local resources (`<Screen.Resources>`: colors, styles) stay in `.yui`. Shared UI styles in `.yres` are a possible later entry type (`style`); not in the current format.

## 7. Generated code shape

### 7.1 Small example
`src/counter.yui`:
```xml
<Screen id="demo:counter" width="120" height="60" x:ViewModel="CounterVm" padding="6">
    <Column w="100%" gap="4">
        <Label text="{x:Bind Strings::clicks(count)}" d:text="Clicked 3 times"/>
        <Button w="50" h="16" text="@res:Strings.add" command="{x:Bind increment()}"
                enabled="{x:Bind can_increment()}" bg="#FF555555" hover:bg="#FF777777"/>
    </Column>
</Screen>
```
`obj/generated/counter.g.rs`:
```rust
// <auto-generated> from src/counter.yui (codegen 1) — do not edit </auto-generated>
const _: () = ::yog_kit::__codegen::require(1);
#[allow(unused_imports)] use crate::res::*;

#[allow(dead_code)] pub const ID: &str = "demo:counter";
#[allow(dead_code)] pub const WIDTH: f32 = 120.0;
#[allow(dead_code)] pub const HEIGHT: f32 = 60.0;
#[allow(dead_code)] pub mod ids {}

#[allow(non_snake_case)]
pub struct View {
    root: ::yog_api::ui::Widget,
    parts: ::yog_kit::Parts,
    b0: ::yog_kit::bind::Slot<String>,   // counter.yui:3:16 text
    b1: ::yog_kit::bind::Slot<String>,   // counter.yui:4:39 text (@res:Strings.add)
    b2: ::yog_kit::bind::Slot<bool>,     // counter.yui:5:17 enabled
}

impl ::yog_kit::View for View {
    type Vm = CounterVm;
    const META: ::yog_kit::ScreenMeta = ::yog_kit::ScreenMeta { id: ID, w: WIDTH, h: HEIGHT,
        anchor: ::yog_kit::Anchor::Center, modal: true, pause: false };

    fn build() -> Self {
        use ::yog_api::ui::{widget, FlexDir, Units, Action, StateFlags, StylePatch};
        let root = widget::panel(FlexDir::Column).w(Units::Px(120.0)).h(Units::Px(60.0)).padding(6.0, 6.0, 6.0, 6.0)
            .child(widget::panel(FlexDir::Column).at(0.0, 0.0).w(Units::Percent(100.0)).gap(4.0)
                .child(widget::label(""))
                .child(widget::button("").w(Units::Px(50.0)).h(Units::Px(16.0)).bg(0xFF555555)
                    .state_style(StateFlags::HOVER, StylePatch { bg: Some(0xFF777777), ..Default::default() })
                    .action(Action { owner: 0, slot: 0, index: 0 })));
        let mut parts = ::yog_kit::Parts::new();
        parts.focusable(&[0, 1]);
        View { root, parts, b0: ::yog_kit::bind::Slot::new(), b1: ::yog_kit::bind::Slot::new(), b2: ::yog_kit::bind::Slot::new() }
    }
    fn root(&self) -> &::yog_api::ui::Widget { &self.root }
    fn root_mut(&mut self) -> &mut ::yog_api::ui::Widget { &mut self.root }
    fn parts(&mut self) -> &mut ::yog_kit::Parts { &mut self.parts }

    fn update(&mut self, vm: &CounterVm, u: &mut ::yog_kit::Update) {
        // @yui counter.yui:3:16 text="{x:Bind Strings::clicks(count)}"
        if self.b0.set(::yog_kit::bind::text(&Strings::clicks(&vm.count))) {
            self.root.at_mut(&[0, 0]).set_text(self.b0.get().clone()); u.relayout();
        }
        // @yui counter.yui:4:39 text="@res:Strings.add"
        if self.b1.set(::yog_kit::bind::text(&Strings::add())) {
            self.root.at_mut(&[0, 1]).set_text(self.b1.get().clone()); u.relayout();
        }
        // @yui counter.yui:5:17 enabled="{x:Bind can_increment()}"
        if self.b2.set(::yog_kit::bind::flag(vm.can_increment())) {
            self.root.at_mut(&[0, 1]).enabled = *self.b2.get(); u.repaint();
        }
    }

    fn dispatch(&mut self, vm: &mut CounterVm, cx: &mut ::yog_kit::Cx<'_, CounterVm>, ev: ::yog_kit::ViewEvent) {
        let _ = cx;
        match ev {
            // @yui counter.yui:4:62 command="{x:Bind increment()}"
            ::yog_kit::ViewEvent::Action(::yog_api::ui::Action { owner: 0, slot: 0, .. }) => { vm.increment(); }
            _ => {}
        }
    }
}

static __HANDLE: ::std::sync::OnceLock<::yog_kit::ScreenHandle<CounterVm>> = ::std::sync::OnceLock::new();
/// Registers the screen (render + input) with a fresh view model.
#[allow(dead_code)]
pub fn register(registry: &mut ::yog_api::Registry) -> ::yog_kit::ScreenHandle<CounterVm> {
    register_with(registry, <CounterVm as ::std::default::Default>::default())   // or x:Create expr
}
#[allow(dead_code)]
pub fn register_with(registry: &mut ::yog_api::Registry, vm: CounterVm) -> ::yog_kit::ScreenHandle<CounterVm> {
    __HANDLE.get_or_init(|| ::yog_kit::Screen::<View>::register(registry, vm)).clone()
}
#[allow(dead_code)] pub fn handle() -> Option<::yog_kit::ScreenHandle<CounterVm>> { __HANDLE.get().cloned() }
#[allow(dead_code)] pub fn open() { ::yog_api::open_ui(ID, true, false); }
#[allow(dead_code)] pub fn close() { ::yog_api::close_ui(ID); }
```
`Strings::clicks(&vm.count)`: arguments to TypePath calls are passed by reference when they are VM paths. `&T: Display` holds whenever `T: Display`, so this avoids moving out of `vm`. The codegen emits `&` for path arguments of TypePath calls, and the arguments of VM method calls verbatim.

Code-behind:
```rust
yog_api::ui_include!("counter");
use yog_kit::prelude::*;
#[derive(Default)] pub struct CounterVm { count: u32 }
impl ViewModel for CounterVm {}
impl CounterVm { fn increment(&mut self) { self.count += 1 } fn can_increment(&self) -> bool { self.count < 10 } }
```

### 7.2 List excerpt (Modlist ListView)
The template instance struct and the update block:
```rust
pub struct __T0 { b0: Slot<String>, b1: Slot<String> }           // ModEntry row template
impl __T0 { fn build() -> (Self, Widget) { /* row panel, ellipsis labels, selected:/hover: patches */ } }
// in View: l0: ::yog_kit::items::ItemsHost<__T0>, created as ItemsHost::new(1, &[0, 1, 0, 2], Some(24.0))

// @yui mod_list.yui:58:9 ListView items="{x:Bind visible()}" key="{x:Bind id}" selected="{x:Bind selected_id(), …}"
{
    let sel = vm.selected_id();
    let mut it = self.l0.begin(&mut self.root, u);
    for (index, item) in ::std::iter::IntoIterator::into_iter(vm.visible()).enumerate() {
        let _ = index;
        let Some((inst, w)) = it.next(::yog_kit::bind::key(&item.id), __T0::build) else { continue };
        it.set_selected(::yog_kit::bind::is_selected(&sel, &item.id));
        // @yui mod_list.yui:63:25 text="{x:Bind name}"
        if inst.b0.set(::yog_kit::bind::text(&item.name)) { w.at_mut(&[0]).set_text(inst.b0.get().clone()); u.relayout(); }
        // @yui mod_list.yui:66:25 text="{x:Bind subtitle()}"
        if inst.b1.set(::yog_kit::bind::text(&item.subtitle())) { w.at_mut(&[1]).set_text(inst.b1.get().clone()); u.relayout(); }
    }
    it.finish(Some(|| __E0::build_static()));   // <ListView.Empty>
}
// dispatch:
::yog_kit::ViewEvent::Action(Action { owner: 1, slot: ::yog_kit::items::SELECT, index })
| ::yog_kit::ViewEvent::Select { list: 1, index } => {
    // @yui mod_list.yui:58:9 selected BindBack=select
    let k = ::std::iter::IntoIterator::into_iter(vm.visible()).nth(index as usize)
        .map(|item| ::std::clone::Clone::clone(&item.id));
    if let Some(k) = k { vm.select(k); }
}
```
Template bodies are inlined into the screen's `update`/`dispatch`, so `item`'s type is inferred and never named. The rule is that `items` must be a pure function of VM state; this is documented.

### 7.3 Source maps and diagnostics
- **Comments and map file.** Every binding block starts with `// @yui file:line:col attr="…"`. The generator writes through a `LineWriter` that records `g.rs line → (yui line, col, attr)`, saved as `<name>.g.map` (JSON lines) next to the `.g.rs`.
- **Remapping.** `yog build` / `yog check` run cargo with `--message-format=json-diagnostic-rendered-ansi`. For each diagnostic whose primary span is in `generated/*.g.rs`, they print `src/mod_list.yui:63:25: error[E0609]: no field `nme` on type `&ModEntry` (in text="{x:Bind nme}")`, followed by the original rendered message as a note. Other diagnostics pass through unchanged.
- **Early errors.** The codegen itself reports, with `.yui` line numbers and before cargo runs: unknown attributes, a binding on a non-bindable attribute, TwoWay on a non-place expression, unknown `@res:Set.name` (it reads `.yres`), state prefixes on layout attributes, and missing `x:ViewModel` when bindings are used.
- **rust-analyzer** sees the real `.g.rs` (in-flight setup). IDLE maps its squiggles back through `.g.map`.

---

## 8. Yog-Modlist rewritten

**Tree:** `resources/Strings.yres`, `resources/Strings.ru_ru.yres`, `resources/Icons.yres` (+ `icons/logo.png`), `src/lib.rs`, `src/mod_list.yui`, `src/mod_list.rs`, `src/mod_list.sample.json`. Any earlier `lang/*.ylang` files are replaced by these `.yres` files.

`yog.toml` (excerpt):
```toml
[mod]
id = "yog-modlist"
name = "@res:Strings.mod_name"
description = "@res:Strings.mod_description"
icon = "@res:Icons.logo"
version = "0.3.0"
[dependencies]
yog-kit = "0.2"
```

`resources/Strings.yres` (excerpt; `key` keeps the existing lang keys so translations carry over):
```json
{
  "mod_name": "Yog Mod List",
  "mod_description": "In-game mod list browser.",
  "menu_button": { "value": "Yog Mods", "key": "yog-modlist.menu.button" },
  "title": { "value": "Yog Mods", "key": "yog-modlist.screen.title" },
  "count": { "value": "{count} mods", "key": "yog-modlist.screen.count", "comment": "{count}: installed mods" },
  "count_filtered": { "value": "{shown} of {total} mods", "key": "yog-modlist.screen.count_filtered" },
  "close": { "value": "Close", "key": "yog-modlist.screen.close" },
  "search_placeholder": { "value": "Search mods…", "key": "yog-modlist.search.placeholder" },
  "filter_all": { "value": "All", "key": "yog-modlist.filter.all" },
  "filter_yog": { "value": "Yog", "key": "yog-modlist.filter.yog" },
  "filter_platform": { "value": "Platform", "key": "yog-modlist.filter.platform" },
  "list_empty": { "value": "No mods", "key": "yog-modlist.list.empty" },
  "list_no_matches": { "value": "Nothing matches the search", "key": "yog-modlist.list.no_matches" },
  "badge_yog": { "value": "[Yog]", "key": "yog-modlist.badge.yog" },
  "badge_platform": { "value": "[MC]", "key": "yog-modlist.badge.platform" },
  "row_version": { "value": "v{version}", "key": "yog-modlist.row.version" },
  "source_yog": { "value": "Yog mod", "key": "yog-modlist.source.yog" },
  "source_platform": { "value": "Platform mod", "key": "yog-modlist.source.platform" },
  "details_version": { "value": "Version {version}", "key": "yog-modlist.details.version" },
  "details_authors": { "value": "by {authors}", "key": "yog-modlist.details.authors" },
  "details_authors_unknown": { "value": "by unknown", "key": "yog-modlist.details.authors_unknown" },
  "details_id": { "value": "id: {id}", "key": "yog-modlist.details.id" },
  "details_no_description": { "value": "No description.", "key": "yog-modlist.details.no_description" },
  "details_none_selected": { "value": "No mod selected", "key": "yog-modlist.details.none_selected" }
}
```

`src/mod_list.yui`:
```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- The mod browser. View model: ModListVm in mod_list.rs. -->
<Screen id="yog-modlist:mod_list"
        width="400"
        height="224"
        anchor="center"
        bg="#E0101010"
        x:ViewModel="ModListVm"
        d:Sample="mod_list.sample.json">
    <Screen.Resources>
        <Color x:Key="Accent">#FFD700</Color>
        <Color x:Key="Pane">#88111111</Color>
        <Style x:Key="Filter" target="ToggleButton" w="58" h="14" bg="#FF333333" color="#FFFFFF"
               hover:bg="#FF444444" checked:bg="#FF555555" disabled:color="#FF777777"/>
        <Style x:Key="Detail" target="Label" color="#CCCCCC" shadow="false"/>
    </Screen.Resources>
    <Column w="100%" h="100%" gap="0">
        <Row w="100%" h="20" padding="0 8" gap="4" align="center" bg="#FF222222">
            <Image texture="@res:Icons.logo" w="12" h="12"/>
            <Label text="@res:Strings.title" color="@Accent" no_wrap="true"/>
            <Spacer flex="1"/>
            <Label text="{x:Bind count_text()}" color="#AAAAAA" shadow="false" no_wrap="true" d:text="58 mods"/>
            <Button w="40" h="14" text="@res:Strings.close" bg="#FF333333" hover:bg="#FF555555"
                    command="{x:Bind cx.close()}"/>
        </Row>
        <Row flex="1" w="100%" padding="6 8 8 8" gap="6">
            <Column w="50%" h="100%" gap="4">
                <TextBox w="100%" h="14" text="{x:Bind search, Mode=TwoWay}"
                         placeholder="@res:Strings.search_placeholder"/>
                <Row w="100%" h="14" gap="3">
                    <ToggleButton style="@Filter" text="@res:Strings.filter_all"
                                  checked="{x:Bind filter == Filter::All}"
                                  command="{x:Bind set_filter(Filter::All)}"/>
                    <ToggleButton style="@Filter" text="@res:Strings.filter_yog"
                                  checked="{x:Bind filter == Filter::Yog}"
                                  enabled="{x:Bind has(Filter::Yog)}"
                                  command="{x:Bind set_filter(Filter::Yog)}"/>
                    <ToggleButton style="@Filter" text="@res:Strings.filter_platform"
                                  checked="{x:Bind filter == Filter::Platform}"
                                  enabled="{x:Bind has(Filter::Platform)}"
                                  command="{x:Bind set_filter(Filter::Platform)}"/>
                </Row>
                <ListView w="100%" flex="1" gap="2" padding="2" bg="@Pane"
                          items="{x:Bind visible()}"
                          key="{x:Bind id}"
                          selected="{x:Bind selected_id(), BindBack=select}"
                          scroll_key="{x:Bind list_scope()}"
                          item_height="24"
                          d:items="8">
                    <ListView.ItemTemplate>
                        <DataTemplate x:DataType="ModEntry">
                            <Column w="100%" padding="3 4" gap="1" bg="#44333333"
                                    hover:bg="#66333344" selected:bg="#883A3A5A">
                                <Label w="100%" text="{x:Bind name}" color="#FFFFFF"
                                       selected:color="@Accent" ellipsis="true"/>
                                <Label w="100%" text="{x:Bind subtitle()}" color="#AAAAAA"
                                       shadow="false" ellipsis="true"/>
                            </Column>
                        </DataTemplate>
                    </ListView.ItemTemplate>
                    <ListView.Empty>
                        <Label text="{x:Bind empty_text()}" color="#888888" shadow="false"/>
                    </ListView.Empty>
                </ListView>
            </Column>
            <ScrollView flex="1" h="100%" padding="4 6" bg="@Pane" scroll_key="{x:Bind selected_id()}">
                <ContentPresenter w="100%" content="{x:Bind selected()}">
                    <DataTemplate x:DataType="ModEntry">
                        <Column w="100%" gap="3">
                            <Label text="{x:Bind name}" color="#FFFFFF"/>
                            <Label style="@Detail" text="{x:Bind Strings::details_version(version)}" color="#AAAAAA"/>
                            <Label style="@Detail" text="{x:Bind source.label()}" color="{x:Bind source.color()}"/>
                            <Label style="@Detail" text="{x:Bind authors_text()}"/>
                            <Label style="@Detail" text="{x:Bind Strings::details_id(id)}" color="#9999BB"/>
                            <Spacer h="4"/>
                            <Label style="@Detail" text="{x:Bind description_text()}" color="#DDDDDD"/>
                        </Column>
                    </DataTemplate>
                    <ContentPresenter.Empty>
                        <Label text="@res:Strings.details_none_selected" color="#FFFFFF"/>
                    </ContentPresenter.Empty>
                </ContentPresenter>
            </ScrollView>
        </Row>
    </Column>
</Screen>
```

`src/mod_list.rs` (the whole file):
```rust
//! View model of the mod browser (`mod_list.yui`). Layout, bindings, scrolling,
//! selection and keyboard handling come from the markup and yog-kit.

yog_api::ui_include!("mod_list");

use yog_kit::prelude::*;
use crate::res::Strings;

#[derive(Debug, Clone)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub authors: String,
    pub description: String,
    pub source: ModSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSource { Yog, Platform }

impl ModSource {
    pub fn label(self) -> String {
        match self { Self::Yog => Strings::source_yog(), Self::Platform => Strings::source_platform() }
    }
    pub fn color(self) -> u32 {
        match self { Self::Yog => 0xFF_88FF88, Self::Platform => 0xFF_88BBFF }
    }
    fn badge(self) -> String {
        match self { Self::Yog => Strings::badge_yog(), Self::Platform => Strings::badge_platform() }
    }
}

impl ModEntry {
    pub fn subtitle(&self) -> String {
        format!("{}  {}", Strings::row_version(&self.version), self.source.badge())
    }
    pub fn authors_text(&self) -> String {
        if self.authors.is_empty() { Strings::details_authors_unknown() } else { Strings::details_authors(&self.authors) }
    }
    pub fn description_text(&self) -> String {
        if self.description.is_empty() { Strings::details_no_description() } else { self.description.clone() }
    }
    fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || [&self.name, &self.id, &self.authors].iter().any(|s| s.to_lowercase().contains(query))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Filter { #[default] All, Yog, Platform }

impl Filter {
    fn allows(self, e: &ModEntry) -> bool {
        match self {
            Filter::All => true,
            Filter::Yog => e.source == ModSource::Yog,
            Filter::Platform => e.source == ModSource::Platform,
        }
    }
}

#[derive(Default)]
pub struct ModListVm {
    mods: Vec<ModEntry>,
    filter: Filter,
    search: String,
    /// Chosen mod; `None` = the first one shown.
    selected: Option<String>,
}

impl ViewModel for ModListVm {
    /// Every mod has loaded by the time the screen first opens.
    fn on_open(&mut self, _cx: &mut Cx<'_, Self>) {
        if self.mods.is_empty() {
            self.mods = load_mods();
        }
    }
}

impl ModListVm {
    pub fn visible(&self) -> impl Iterator<Item = &ModEntry> + '_ {
        let query = self.search.trim().to_lowercase();
        self.mods.iter().filter(move |m| self.filter.allows(m) && m.matches(&query))
    }
    pub fn has(&self, f: Filter) -> bool { self.mods.iter().any(|m| f.allows(m)) }
    pub fn count_text(&self) -> String {
        let (shown, total) = (self.visible().count(), self.mods.len());
        if shown == total { Strings::count(total) } else { Strings::count_filtered(shown, total) }
    }
    pub fn empty_text(&self) -> String {
        if self.search.trim().is_empty() { Strings::list_empty() } else { Strings::list_no_matches() }
    }
    pub fn selected(&self) -> Option<&ModEntry> {
        match &self.selected {
            Some(id) => self.visible().find(|m| &m.id == id),
            None => self.visible().next(),
        }
    }
    pub fn selected_id(&self) -> Option<&str> { self.selected().map(|m| m.id.as_str()) }
    pub fn list_scope(&self) -> (Filter, &str) { (self.filter, self.search.as_str()) }

    pub fn select(&mut self, id: String) { self.selected = Some(id); }
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
        if self.selected().is_none() { self.selected = None; }   // keep it if still shown
    }
}

/// Yog mods first, then by name.
fn load_mods() -> Vec<ModEntry> {
    let mut mods: Vec<ModEntry> = yog_api::installed_mods().into_iter().map(|m| ModEntry {
        source: if m.source == "yog" { ModSource::Yog } else { ModSource::Platform },
        id: m.id, name: m.name, version: m.version, authors: m.authors, description: m.description,
    }).collect();
    mods.sort_by_key(|e| (e.source != ModSource::Yog, e.name.to_lowercase()));
    mods
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_kit::testing::TestHost;
    fn vm() -> ModListVm { /* two Yog + one Platform ModEntry */ unimplemented!() }
    #[test] fn filter_keeps_selection_when_still_shown() { /* pure VM test */ }
    #[test] fn clicking_filter_updates_count() {
        let mut h = TestHost::<View>::new(vm()); h.size(400.0, 224.0); h.frame();
        // click the Yog toggle by path / id, assert h.vm().filter == Filter::Yog and the count label text
    }
}
```
Bindings that receive a template-root path are passed by reference: `Strings::details_version(version)` becomes `Strings::details_version(&item.version)`, and `&String: Display` holds.

`src/lib.rs`:
```rust
mod mod_list;
yog_api::resources!();   // mod res { Strings, Icons, … } from resources/*.yres

use yog_api::{info, Mod, Registry};
use res::Strings;

pub struct YogModList;
impl Mod for YogModList {
    fn register(registry: &mut Registry) {
        mod_list::register(registry);
        registry.register_menu_entry(&Strings::menu_button(), mod_list::ID);
        info!("[yog-modlist] ready.");
    }
}
yog_api::export_mod!(YogModList);
```

**What disappears:**
- the global `Mutex` / `OnceLock` state;
- `find_mut` / `set`;
- manual filter styling;
- `ScrollState` routing;
- `"select:<i>"` string events;
- manual row building and `on_event` parsing.

That is about 330 lines of code-behind down to about 120, and every remaining line is domain logic. Search, close, keyboard navigation and virtualization come for free.

---

## 9. Project scaffolding: one template source for CLI and IDE

**Single source.**
- `rust/crates/yog-cli/templates/<id>/` holds a `template.toml` plus files with `{{placeholders}}`.
- `build.rs` embeds the directory, as a generated `templates_embedded.rs` of `(path, include_bytes!)` entries.
- `src/templates.rs` renders templates.
- IDLE never hard-codes Yog templates. It calls:
  - `yog templates list [--kind project|item] --json` → `[{id, kind, category, name, description, extension, base_name, default}]`
  - `yog templates render <id> --name <stem> [--dir <src-subdir>] [--mod-id <id>] --json` → `{files: [{path, contents}], appends: [{path, before, text}], lib_rs: {mods: [...], register: [...]}, hints: [...]}` (dry run; IDLE shows it, then writes)
  - `yog new <name> [--template app|content|empty] [--json]`
  - `yog item new <template> <name> [--dir src/screens]`, which writes files and applies appends and `lib.rs` edits.

  IDLE's generic non-mod templates (JSON, TOML, Markdown, Text, XML) may stay in TS; every "Yog" category template comes from yog-cli.

**Placeholders:**
- `{{mod_id}}`, `{{mod_name}}`, `{{ModType}}`
- `{{stem}}`, `{{Stem}}` (PascalCase), `{{ui_name}}` (include path, e.g. `screens/options`), `{{ui_id}}`
- `{{author}}` (git config, else "Your Name"), `{{year}}`

**`template.toml` schema:**
```toml
id = "screen"; kind = "item"; category = "Yog"; name = "Screen (MVVM)"
description = "A .yui screen with its view model; bindings are generated into obj."
extension = ".yui"; base_name = "screen"
[[file]]   src = "screen.yui"   dest = "src/{{dir}}{{stem}}.yui"
[[file]]   src = "screen.rs"    dest = "src/{{dir}}{{stem}}.rs"
[[json_entry]] dest = "resources/Strings.yres" name = "{{stem}}_title" value = "{{Stem}}"   # inserted into the JSON object
[lib_rs]   mods = ["{{stem}}"]  register = ["{{stem}}::register(registry);"]
```
`lib.rs` edits use syn to locate the last `mod` item and the body of `fn register` in `impl Mod`. This is the same approach as IDLE's `designer/codebehind.rs`; that logic moves to a small shared module in yog-cli. If parsing fails, the edit falls back to a printed hint.

### Project templates

**`app` (default for `yog new`): mod with an MVVM screen.**
```
my-mod/
├─ yog.toml                  # name/description/icon = @Strings.*/@res:Icons.logo, [dependencies] yog-kit, [resources] neutral
├─ README.md
├─ .gitignore                # obj/  target/  bin/
├─ resources/
│  ├─ Strings.yres           # mod_name, mod_description, menu_button, title, clicks ({count}), add, reset, close
│  ├─ Strings.ru_ru.yres     # the same names, Russian values (example culture)
│  ├─ Icons.yres             # { "logo": { "type": "image", "value": "icons/logo.png" } }
│  └─ icons/logo.png         # 16×16 placeholder, embedded bytes in the template
└─ src/
   ├─ lib.rs                 # mod main_screen; mod res; register + menu entry
   ├─ main_screen.yui        # Image @res:Icons.logo, @res:Strings.title, {x:Bind Strings::clicks(count)}, 3 commands, hover: states
   ├─ main_screen.rs         # MainVm { count } + increment/reset/can_reset, tests with TestHost
   └─ main_screen.sample.json
```

`src/lib.rs`:
```rust
mod main_screen;
yog_api::resources!();   // mod res { Strings, Icons, … } from resources/*.yres

use yog_api::{Mod, Registry};
use res::Strings;

pub struct {{ModType}};
impl Mod for {{ModType}} {
    fn register(registry: &mut Registry) {
        main_screen::register(registry);
        registry.register_menu_entry(&Strings::menu_button(), main_screen::ID);
    }
}
yog_api::export_mod!({{ModType}});
```

`src/main_screen.yui`:
```xml
<?xml version="1.0" encoding="utf-8"?>
<Screen id="{{mod_id}}:main" width="176" height="96" anchor="center" bg="#C0101010" padding="8"
        x:ViewModel="MainVm" d:Sample="main_screen.sample.json">
    <Column w="100%" h="100%" gap="4">
        <Row w="100%" gap="4" align="center">
            <Image texture="@res:Icons.logo" w="16" h="16"/>
            <Label text="@res:Strings.title" color="#FFFFFF" no_wrap="true"/>
        </Row>
        <Label text="{x:Bind Strings::clicks(count)}" color="#CCCCCC" d:text="Clicked 3 times"/>
        <Row gap="4">
            <Button w="60" h="16" text="@res:Strings.add" bg="#FF555555" hover:bg="#FF777777"
                    command="{x:Bind increment()}"/>
            <Button w="60" h="16" text="@res:Strings.reset" bg="#FF555555" hover:bg="#FF777777"
                    disabled:color="#FF777777" enabled="{x:Bind can_reset()}" command="{x:Bind reset()}"/>
        </Row>
        <Spacer flex="1"/>
        <Button w="100%" h="16" text="@res:Strings.close" bg="#FF333333" hover:bg="#FF555555"
                command="{x:Bind cx.close()}"/>
    </Column>
</Screen>
```

`src/main_screen.rs`:
```rust
//! View model of `main_screen.yui`; the bindings are generated into
//! `obj/generated/main_screen.g.rs` by `yog build` / Yog-IDLE.
yog_api::ui_include!("main_screen");

use yog_kit::prelude::*;

#[derive(Default)]
pub struct MainVm { count: u32 }

impl ViewModel for MainVm {}

impl MainVm {
    fn increment(&mut self) { self.count += 1; }
    fn reset(&mut self) { self.count = 0; }
    fn can_reset(&self) -> bool { self.count > 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_clicks() {
        let mut vm = MainVm::default();
        vm.increment();
        assert!(vm.can_reset());
    }
}
```

`README.md` covers:
- the layout (resources/, src/, generated code in obj);
- `yog build` / `yog run`;
- editing screens in Yog-IDLE;
- adding a language (copy `Strings.yres` to `Strings.<culture>.yres`);
- adding items with `yog item new item <name>`;
- links to docs/yui.md, docs/mvvm.md and docs/resources.md.

**`content`: `app` plus items and blocks.**
- Adds `resources/Textures.yres` with `<Image name="ruby" src="textures/ruby.png" for="item:ruby"/>` and `<Image name="ruby_block" src="textures/ruby_block.png" for="block:ruby_block"/>`, plus the two PNGs.
- `Strings.yres` and `Strings.ru_ru.yres` gain `<String name="ruby" for="item:ruby">` and `<String name="ruby_block" for="block:ruby_block">`.
- `src/content.rs`: `pub fn register(r: &mut Registry) { r.register_item(ItemDef::new("ruby")); r.register_block(BlockDef::new("ruby_block")); }`.
- `lib.rs` calls `content::register`.
- The main screen shows a ruby `ItemSlot item="{{mod_id}}:ruby"`.

**`empty`.** yog.toml with resources, `resources/Strings.yres` + `Strings.ru_ru.yres` + `Icons.yres` (+ logo), and `lib.rs` with `yog_api::resources!();` and an empty register. It has no screen and no yog-kit dependency.

### Item templates (Add → New Item, and `yog item new`)

| id | Creates | Appends / lib.rs |
|---|---|---|
| `screen` | `<stem>.yui` (VM screen, `@Strings.<stem>_title`, close command), `<stem>.rs` (`<Stem>Vm`) | Strings `<stem>_title`; `mod`, `register` |
| `list-screen` | screen with ListView + ItemTemplate + details ContentPresenter + `<Stem>Vm` holding `Vec<Entry>` | Strings `<stem>_title`, `<stem>_empty` |
| `dialog` | `<stem>.yui` with a `<Dialog>` root, `<Stem>Vm` with ok/cancel | Strings `<stem>_title`, ok, cancel |
| `item` | `src/items/<stem>.rs` registering `ItemDef::new("<stem>")` | Strings `for="item:<stem>"`, Textures Image `for=` + placeholder PNG |
| `block` | same, for blocks | same |
| `resource-strings` | `resources/<Stem>.yres` (String sample) | none |
| `resource-culture` | `resources/<Set>.<culture>.yres` with every String name of the neutral file, values copied, marked `comment="TODO translate"` | none |
| `resource-images` | `resources/<Stem>.yres` with an Image sample | none |
| `module` | `<stem>.rs` with `register` | `mod`, `register` |

In IDLE, the existing "Language File" template (en_us.json) is replaced by `resource-culture`.

**IDLE New Project.** IDLE currently has no wizard. Add `src/panels/NewProjectDialog.tsx`:
- template cards from `yog templates list --kind project --json`;
- name, mod id and location fields;
- runs `yog new --template <id> --json <path>` through a new tauri command in `src-tauri/src/project_new.rs`;
- opens the folder.

It is reachable from the File menu and from `EmptyEditorState`.

`NewItemDialog.tsx`:
- `ITEM_TEMPLATES` for the "Yog" category is replaced by a fetched list (tauri `templates_list`, cached per session);
- `templateFiles` calls `templates_render`;
- `onAdd` writes the files and applies appends and `lib.rs` edits as returned.

**Drift tests:**
- yog-cli: every template renders, and `yog new -t <each>` followed by `yog check` passes (`#[ignore]`d unless `YOG_E2E=1`).
- Every item template applied onto `app` passes `yog check`.
- Snapshot tests of `templates render --json`.
- IDLE has no copies to drift.

---

## 10. Migration and compatibility

- **Screens without `x:ViewModel`** generate byte-identical output to today, through `gen_legacy.rs`. A golden test pins this, and existing code-behind keeps working.
- **VM screens** still emit `ID`, `ids`, `open()`, `close()`. `build/render_with/click/with_last` are not emitted because the host owns them. `ViewModel::customize` and `cx.focus(ids::X)` remain as escape hatches.
- **No incremental migration.** A screen is either plain or VM; adding `x:ViewModel` means replacing every `on_click`/`on_change`/`on_submit` with `command=` / `Mode=TwoWay` (they're errors in VM screens).
- **`@mod.key` raw keys** keep working. `.ylang` files are superseded by `.yres` (existing keys kept via the `key` field).
- **Reserved names** in VM-screen modules: `View`, `ID`, `WIDTH`, `HEIGHT`, `ids`, `register`, `register_with`, `handle`, `open`, `close`, `__T*`, `__E*`, `__HANDLE`.
- **yog-ui struct changes** are additive fields with defaults. They break only external struct-literal construction of `Widget`/`LayoutNode`; none is known (grep the repos before release). Bump the minor version.
- **Host event strings.** `move`, `open` and `close` are new prefixes. Old handlers ignore them: `ScrollState::handle` returns false, `click()` returns None, and TextBoxState ignores them.
  - yog-kit requires a host that sends them (runtime with `nativeUIMove` and open/close events); there is no fallback for older hosts.

---

## 11. Designer and IDE integration (Yog-IDLE)

1. **Model.**
   - `src/designer/yui.ts`: add the kinds `ToggleButton`, `ListView`, `ItemsControl`, `ContentPresenter`, `DataTemplate`, `Dialog`, and property elements (`X.Y` children).
   - `properties.ts`:
     - each `AttrSpec` gains `bindable: true`, `stateful: boolean` (paint attributes);
     - new specs for `command`, `items`, `key`, `selected`, `checked`, `visible`, `scroll_key`, `item_height`, `style`, `x:ViewModel`, `x:Create`, `x:DataType`, `d:Sample`.
2. **Design-time rendering** (`src-tauri/src/designer/markup.rs`, which builds a real `yog_ui::Widget`). For each attribute value:
   - literal → as today;
   - `@res:Set.name` → read `resources/*.yres` (neutral culture, or the culture picked in a toolbar dropdown);
   - `{x:Bind e}` → in order: `d:<attr>`, then `sample.json[e]` (template scope: `sample[items_expr][i][e]`), then a placeholder `‹e›` in grey for text, and default values for flags and colors;
   - `ItemsControl` → `d:items` copies, or the sample array;
   - `ContentPresenter` → template if a sample exists, else Empty.
   - The canvas gets a state preview toggle (Normal / Hover / Pressed / Selected / Checked / Disabled) that sets `StateFlags` on the selected widget.
3. **VM schema.**
   - New `yog ui schema <file.yui> --json` in yog-cli `yui/schema.rs`. It parses the code-behind (`<stem>.rs`) with syn: struct fields of `x:ViewModel` with types, inherent methods with arity and return types, `impl ViewModel` presence, the field/method list of the item type named by `x:DataType`, and resource sets and names from `.yres`.
   - IDLE calls it on save and on code-behind change.
4. **Properties panel.**
   - A bound attribute shows a chip `{x:Bind count_text()}` with a ⋯ button that opens the **Binding Editor**:
     - source tree (VM members, or item members inside templates; methods show arity);
     - Mode, BindBack (filtered to `fn(&mut self, T)`), Fallback, and a preview value from the sample;
     - writes the markup as a patch.
   - The **Resource picker** for text, color and texture lists `@Strings.*` etc., with a "new string…" action that appends to `Strings.yres`.
   - A **State** dropdown in the Appearance category edits `hover:`/`selected:`… variants of paint attributes.
5. **XAML editor.** Completion inside `{x:Bind |}` from the schema; inside `@` (resource sets and names); inside `style="@` (styles). Hover shows the type and value from the sample.
6. **Double-click** on a Button or ToggleButton in a VM screen (extending `codebehind.rs`) adds `fn <name>(&mut self) {}` to `impl <Vm>` if missing, sets `command="{x:Bind <name>()}"`, and navigates there.
7. **Diagnostics.** IDLE runs `yog check` with JSON output. Remapped `.yui` diagnostics become squiggles in the XAML pane. `.g.map` is used for rust-analyzer diagnostics in `.g.rs`.
8. **`.yres` editor.** A grid tab: Name | Value per culture column | Comment | Type | `for`. It has an image thumbnail column, adds cultures, and highlights missing translations. Edits patch the XML.
9. **Toolbox:** ToggleButton, ListView, ItemsControl, ContentPresenter. Dropping a ListView inserts a skeleton ItemTemplate.
10. **Live preview (later).** With the game running and hot reload, `ScreenHandle` state is visible in the real game. This is optional and out of scope for v1.

---

## 12. ABI and host impact

- **`yog-abi` `YogApi` table: no change.** All of yog-kit and the generated code are linked into each mod's cdylib.
- **yog-runtime `ui_jni.rs`:**
  - new `Java_dev_yog_NativeBridge_nativeUIMove(uiId, x, y)`, which dispatches `move:{x:.1}:{y:.1}`;
  - `nativeUIShow` additionally dispatches `open:{w}:{h}` to the UI's handler;
  - `nativeUIHide` dispatches `close`.

  These use the same handler map as `click`.
- **Java `YogUIScreen`** (fabric/forge/neoforge × 1.20.1/1.21.1):
  - `render()` calls `NativeBridge.nativeUIMove` when `mx`/`my` changed since the last frame;
  - add `mouseMoved` (forward the same event);
  - declare the native in `NativeBridge`.
- **Items:** verify that the hosts fall back to the translatable `item.<ns>.<id>` / `block.<ns>.<id>` when `ItemDef.name` is `None`. Fix in Java if not.
- **Packaging:** manifest gains `icon`, `name_localized` and `description_localized` (yog-cli `package`). Hosts ignore unknown keys; verify.

---

## 13. Testing strategy

1. **yog-ui** unit tests:
   - `hit_path` / `node_at` mirror widget paths, including with invisible children;
   - `action` hit priority (deeper wins; disabled not hit);
   - `visible=false` takes no space;
   - `effective_style` precedence and inheritance stopping at interactive descendants;
   - `UiEvent::parse` round-trips every host format, with garbage going to `Other`.
2. **yog-cli codegen:**
   - Golden tests in `crates/yog-cli/tests/yui/{legacy,vm}/*.yui` → `*.g.rs` (+ `.g.map`), with `UPDATE_GOLDEN=1` to refresh. Legacy fixtures include today's Modlist markup, which must be byte-identical.
   - Error tests (line and column for each early error).
   - Binding parser and emitter unit tests: scopes, `?.`, `==`, TypePath, reference passing, TwoWay place checks.
   - `.yres` tests: arity, key derivation, `for=` outputs, culture warnings, PNG size.
   - Diagnostic remap tests on recorded cargo JSON.
3. **yog-kit:**
   - Fixtures `tests/fixtures/{counter,list,dialog}.yui` with committed generated `.g.rs`. A yog-cli test asserts they are up to date, so kit tests compile real codegen without a build-dep on the yog binary.
   - Headless `TestHost` tests:
     - click → command → patched text;
     - no update when nothing changed (Slot counters);
     - relayout only on layout changes;
     - list reconcile (insert, remove, reorder keeps instance state);
     - virtualization window and spacer heights;
     - selection via click and arrows plus BindBack;
     - TwoWay TextBox;
     - scroll_key reset;
     - focus cycle and Esc → on_back → close;
     - queued `ScreenHandle::update` from another thread;
     - dialogs and their results.
   - Compile-fail tests (`trybuild` dev-dep) check that the `on_unimplemented` messages appear.
4. **Templates:** render snapshots; `YOG_E2E=1` runs `yog new` for each template plus `yog check`, and each item template onto `app` plus `yog check`.
5. **Modlist:** pure VM tests (filter, search, selection fallback, counts) and TestHost tests (see §8).
6. **IDLE:**
   - TS tests for binding chip parse/serialize and property element handling;
   - Rust tests in `markup.rs` for design-time substitution (d:, sample, placeholder);
   - schema extraction tests on Modlist.
7. **Manual matrix:** Fabric, Forge and NeoForge × 1.20.1 and 1.21.1:
   - hover, wheel routing, open/close hooks, Esc semantics with a focused TextBox, GUI scale changes (relayout), language switch (re-open).

---

## 14. Phased implementation (after the in-flight work lands)

**P0: Prerequisites and verification (small)**
- Confirm `.g.rs` / `ui_include!` and TextBox landed.
- Grep the repos for struct literals of `Widget`/`LayoutNode`.
- Verify the host behaviour for item names and unknown manifest keys.

**P1: yog-ui primitives**
- `crates/yog-ui/src/widget.rs`: `Action`, `StateFlags`, `StylePatch`, `StateStyles`, fields, builders, `at`/`at_mut`, `set_state_inherited`, `effective_style`.
- `layout.rs`: node fields, invisible handling, `hit_test` on action, `hit_path`, `node_at`.
- `render.rs`: effective style, skip invisible.
- New `event.rs` with `UiEvent`; `scroll.rs` / `textbox.rs` gain `handle_event`.
- `lib.rs` re-exports. Tests. Bump the version.
- Mirror the new fields in `Yog-IDLE/src/designer/layout.ts` (visible) so the designer stays a port.

**P2: Host events (parallel with P1)**
- `yog-runtime/src/ui_jni.rs`: `nativeUIMove`, open/close dispatch.
- `*/platforms/*/src/main/java/dev/yog/{YogUIScreen,NativeBridge}.java` in all six platform trees.

**P3: yog-kit runtime core**
- `rust/Cargo.toml` adds the member `crates/yog-kit`.
- `crates/yog-kit/src/{lib.rs, view.rs (ViewModel, View, ScreenMeta, Anchor, ViewEvent), host.rs (Screen, render/on_event/frame, anchoring = today's root_with logic), handle.rs, cx.rs, bind.rs, update.rs, parts.rs (scrollers, textboxes, focus order), input.rs (routing, focus, keyboard), testing.rs, codegen.rs}`.
- A hand-written `View` in `tests/manual_view.rs` proves the runtime before codegen exists.

**P4: Resources — done separately** (see `Yog-IDLE/docs/yres.md`): `yog-cli/src/yres.rs`, `yog res check`, `yog_api::resources!()`, `tr`/`tr_args`, yog.toml `@` resolution. Remaining for yog-kit: `@res:Set.name` in bindings and styles, and the image-source alias in yog-ui if not already covered by the alias agent.

**P5: yog-cli VM codegen (no templates yet)**
- Split `src/yui.rs` into `src/yui/{mod.rs, xml.rs, model.rs, binding.rs, resources.rs (screen-local + .yres lookup), gen_legacy.rs (moved verbatim), gen_vm.rs, sourcemap.rs}`. Golden-test that legacy output is unchanged before adding anything.
- Then add: `x:ViewModel`/`x:Create`, bindings on scalar attributes, commands, TwoWay TextBox/ToggleButton, `scroll_key`, state prefixes, styles/resources, `@res:Set.name`.
- `main.rs`: auto-add the yog-kit dependency with the local patch; cargo JSON diagnostics remap in `check()`/`build()` (lines ~671 and ~1537).

**P6: Items and templates**
- `yog-kit/src/items.rs` (ItemsHost, cursor, virtualization, Empty).
- gen_vm templates: ItemsControl, ListView (selection, keyboard), ContentPresenter.
- Kit fixtures plus golden tests.

**P7: Modlist migration and docs**
- `Yog-Modlist/{yog.toml, resources/*, src/lib.rs, src/mod_list.yui, src/mod_list.rs, src/mod_list.sample.json}`; delete `src/mod_list.designer.rs` and `lang/`.
- Docs: `Yog-IDLE/docs/yui.md` (VM section, grammar), new `docs/mvvm.md` and `docs/resources.md`.

**P8: Scaffolding**
- `yog-cli/templates/**` with `build.rs` embedding.
- `src/templates.rs` (render, appends, `lib_rs` edits via syn, extracted from IDLE `codebehind.rs` logic).
- `main.rs`: `new_mod` uses the `app` template by default; `templates list|render`; `item new`.
- E2E tests.

**P9: IDLE**
- `src-tauri/src/designer/markup.rs` (design-time), `src-tauri/src/yui.rs` (schema and diagnostics calls), new `src-tauri/src/project_new.rs` and `templates.rs` tauri commands.
- `src/designer/{yui.ts, properties.ts, ScreenDesigner.tsx, YuiSplitEditor.tsx}` (binding chips, Binding Editor, state preview, completion).
- New `src/panels/NewProjectDialog.tsx`; `NewItemDialog.tsx` switched to CLI templates.
- `.yres` grid editor under `src/editors/`.

**P10: Navigation and dialogs**
- `yog-kit/src/{nav.rs, overlay.rs, dialogs/*.yui + committed .g.rs}`.
- `<Dialog>` root in gen_vm; `cx.show`.

**P11 (optional)**
- `yog-kit-macros`, only if real patterns appear: for example `#[derive(ViewModel)]` with `#[vm(on_change(field = "hook"))]`, or an observable collection for very large lists.
- Shared theme `.yres` styles in the kit.
- Live preview.

**Dependencies between phases:** P1 → P3 → P5 → P6 → P7. P4 must come before P5's `@Set` handling and before P7/P8. P2 can run anytime before P7's manual testing. P8 needs P5 and P6; P9 needs P5 and P8; P10 needs P6.

---

## 15. Risks and open questions

- **Rust expressions in markup.** The deliberately small grammar keeps the parser simple, but authors will want `&&` and `<`. Mitigation: an error message suggesting a VM method. Possibly allow `and`, `or` and `lt` keywords later.
- **Borrowing.** Templates re-evaluate `items` at dispatch. A non-deterministic `items` (e.g. one using random or time) would select the wrong row. Document this, and as a debug assert, compare the key hash cached at update with the dispatch-time key.
- **Per-update binding evaluation** with expensive computed methods: for example `visible()` is iterated by `count_text`, `items` and `has`. This is acceptable at mod-list scale. If profiling shows hot spots, add `yog_kit::Memo<T>` keyed by inputs, which still needs no macros.
- **State inheritance semantics** (stop at interactive descendants) must match in yog-ui and in IDLE's TS layout port.
- **`register_menu_entry` label** is resolved once in the startup language. A key-based variant would need host support; defer it.
- **Hot reload** loses VM state, because the VM lives in the reloaded cdylib. Acceptable for v1.

### Critical Files for Implementation
- /home/f000nk/Projects/yog/Yog-Mod-Loader/rust/crates/yog-cli/src/yui.rs
- /home/f000nk/Projects/yog/Yog-Mod-Loader/rust/crates/yog-cli/src/main.rs (new_mod ~391, Cargo.toml generation ~930, gather_assets ~1714, cargo invocations ~671/1537)
- /home/f000nk/Projects/yog/Yog-Mod-Loader/rust/crates/yog-ui/src/widget.rs, layout.rs, render.rs (plus scroll.rs and textbox.rs)
- /home/f000nk/Projects/yog/Yog-Mod-Loader/rust/crates/yog-api/src/lib.rs (ui_include!, the new resources!/res) and registry.rs (register_ui / on_ui_render ~1971–2045)
- /home/f000nk/Projects/yog/Yog-Mod-Loader/rust/crates/yog-runtime/src/ui_jni.rs, and fabric|forge|neoforge/platforms/*/src/main/java/dev/yog/YogUIScreen.java
- /home/f000nk/Projects/yog/Yog-Modlist/src/mod_list.yui, mod_list.rs, lib.rs, lang/*.ylang
- /home/f000nk/Projects/yog/Yog-IDLE/src/panels/NewItemDialog.tsx, src/designer/{yui.ts,properties.ts,ScreenDesigner.tsx}, src-tauri/src/designer/{markup.rs,codebehind.rs}, src-tauri/src/yui.rs, docs/yui.md
