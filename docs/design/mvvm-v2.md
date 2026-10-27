# Design: MVVM v2 for mod UI (yog-kit)

Status: design only, nothing implemented. Scope: `yog-kit`, `yog-ui` (small), `yog-api` (small),
the new `yog-macros` crate (the `yog-interop` rename from
[boilerplate-reduction.md](boilerplate-reduction.md) F2), the `yog-cli` `.yui` codegen
(`yui_vm.rs`, `yui.rs`, `yui_component.rs`), Yog-IDLE's designer, and the mods Yog-Modlist and
HexMod-Yog.

This builds on decisions already recorded elsewhere:

- [boilerplate-reduction.md](boilerplate-reduction.md): **D1**, one `#[derive(Codec)]` format for
  interop, network and saves. **D2**, generic events `registry.on::<E>()` / `before::<E>()`.
  **F3**, typed channels `C2S<P>` / `S2C<P>`. **F8**, self-registering screens and
  `menu_entry=`. **F12**, binding private fields directly, a generated `update()`, and
  `TestHost::opened`. This document assumes all of these and doesn't repeat them.
- [materials-and-vanilla-ui.md](materials-and-vanilla-ui.md): materials, the vanilla theme, new
  controls (`CheckBox`, `Slider`, `TabBar`, `Panel`, `tooltip`). **MVVM stays orthogonal to
  styling.** This document only provides *state sources*: interaction flags, validation state,
  and conditional styles. It never says how anything looks.
- [yog-kit.md](yog-kit.md): the v1 plan. v1 is implemented except P10 (navigation and dialogs).

---

## 0. Summary of decisions

| # | Decision | Why |
|---|---|---|
| V1 | **View models stay plain Rust structs with plain fields.** One attribute, `#[view_model]`, goes on the struct and on its `impl` blocks. It generates the `ViewModel` impl, change tracking, commands, lifecycle and message handlers. Nobody writes `notify`, `Signal<T>` or `set_x()`. | Idiomatic Rust, and it's the minimum boilerplate: CommunityToolkit-style source generation without the wrapper types. |
| V2 | **Change tracking is static and conservative.** The macro computes, from method bodies, which fields each method reads and may write (as bitmasks). Generated bindings know their read masks. After a command or message, only bindings whose masks intersect the write mask are re-evaluated. The v1 `Slot` diff stays as the source of truth: tracking only *skips* work, it never decides a value. Debug builds and `TestHost` verify the tracker against a full re-evaluation. | "Automatic dependency tracking" without runtime getters, and it can't produce a stale UI in release silently because tests catch it. |
| V3 | **Computed properties** are `#[computed]` methods, memoized for the duration of a binding pass and across passes while their read mask is clean. | Expensive derived state (filters) is computed once. |
| V4 | **`ObsVec<T>`** is an observable collection with a change log (insert/remove/move/update/reset) that `ItemsHost` consumes directly. Plain `Vec`/iterators keep working through keyed reconciliation. | Fine-grained list updates and cheap virtualization for large or frequently changing lists. |
| V5 | **Commands are VM methods.** `#[command(can = …)]` adds can-execute (auto-bound to `enabled` → `DISABLED` state). `async fn` commands get running/error state and are disabled while they run. Parameters are inline call arguments. Item commands capture their arguments per row, which fixes v1's O(n) re-iteration. | Same markup as v1 (`command="{x:Bind save()}"`), with WPF `ICommand` semantics added. |
| V6 | **Gestures live in markup**: `<Screen.InputBindings>`, element-level `InputBindings`, `gesture=` on buttons, and `activate=` on list rows. | Key bindings are a view concern. The VM stays view-agnostic and testable. |
| V7 | **The binding language grows** to cover `&&`, `\|\|`, comparisons, `??`, indexers, `Format=`, converter pipes (`\|`), `Mode=OneTime/OneWayToSource`, `Delay=`/`Trigger=`, element references (`#search.text`), `data_context=` scopes, and `parent` in nested templates. `Fallback=` is replaced by `??`. | Covers WPF/WinUI binding features with compiled, rustc-checked code. |
| V8 | **Bindings fail at `yog build` with `.yui` positions.** yog-cli checks names against a VM schema before rustc runs ("no field or method `serch` on `ModListVm`; did you mean `search`?"), and rustc errors in `.g.rs` are remapped to `file.yui:line:col`. | Compile-time-checked bindings with readable errors. |
| V9 | **Validation**: `#[validate]` rules on fields and methods, conversion errors from two-way bindings, a new `INVALID` state flag, `{x:Bind errors.name}`, `<ErrorText for="#name"/>`, and `is_valid()` for can-execute. | Per-property errors shown by controls. |
| V10 | **Styling hooks only**: the `INVALID` flag plus **conditional styles** `style:Danger="{x:Bind failed}"` (CSS-class-like toggles of resource styles). There is no trigger or visual-state XML. Data-template selection is a Rust **`match`** on the item: `<DataTemplate match="Iota::Vec3(..)">`. | Orthogonal to materials. Keeps the material ABI's `u8` state bits unchanged except for one bit (§7). |
| V11 | **Navigation and dialogs (P10)**: typed `cx.navigate::<V>()`, dialogs as overlays inside the current screen (`cx.dialog::<V>()`), with both callback and `await` forms. `MessageBox` ships in yog-kit. Navigation results arrive as the `Returned<R>` event. | Finishes P10. |
| V12 | **One event mechanism: `#[on(E)]`**, mirroring D2's `registry.on::<E>()`. It is used for lifecycle (`Opened`, `Closed`, `Back`, `Tick`, …), messenger messages (`publish(msg)`), and `#[recv(CHANNEL)]` for F3 server→client channels. The `ViewModel` trait's hand-written hooks are deleted. | One concept for every input to a VM. Handlers have known write masks (V2), unlike opaque `handle.update(|vm| …)` closures. |
| V13 | **Codec everywhere state crosses a boundary (D1)**: `#[persist]` VM fields (saved client-side), `Synced<T>` server state for container screens, cross-mod messages, and optional VM state carried across hot reload. | One format, as the user decided. |
| V14 | **The boilerplate budget is zero.** A mod author writes Rust only for real logic. Fields and command bodies are logic. Everything else is generated or declared: notification, binding and command wiring, validation plumbing, screen registration and opening (including opening from a server packet), server sync, settings screens, persisted UI state and custom-drawn surfaces. A screen with plain state, forms, lists or settings needs **no hand-written Rust beyond the VM struct**, and sometimes none at all (§3.0, §12.5–12.8). | The user's goal, verbatim: "код модов нужно от бойлерплейта избавить максимально". |
| V15 | **The old non-VM `.yui` path is deleted.** A `<Screen>` without `x:ViewModel` compiles as a VM screen with `Vm = ()`. `on_click`/`on_change`/`on_submit`, `build()`/`render()`/`click()`/`with_last()`/`text_box()` and the `events` module go. Components become **compiled controls** with typed properties, patched in place. The string `Props` path stays for the designer host and for components chosen by name at runtime (§9). The low-level `yog_api::ui` path (`Widget`, `UiRoot`, `register_ui`, `hit_test`/`find_rect`) stays, and every `.yui` still generates `build()` + `ids` for it (§8, capability rule). | The no-backward-compat rule applies, and VM screens cover every use. No mod uses the old path (checked: every `.yui` in the workspace is either a VM screen or a `<Component>`). |

---

## 1. Where v1 stands

Implemented (Yog-Mod-Loader `rust/crates/yog-kit`, `yog-cli/src/yui_vm.rs`):

- `View` trait: `build` / `update` / `dispatch` (`yog-kit/src/lib.rs`). `Host` owns the VM, the tree
  and the input state (`host.rs`). `Screen::register` / `ScreenHandle` / `Cx` handle registration,
  handles and navigation (`screen.rs`). `ItemsHost` provides keyed lists and virtualization
  (`items.rs`). `TestHost` drives screens in tests (`testing.rs`).
- Every `&mut` access sets `dirty = true`, and `run_update` then re-evaluates **every** binding.
  `Slot<T>` patches only the widgets whose values changed (`bind.rs`).
- Binding language: paths, calls, `Type::f()`, literals, `!`, `==`, `!=`, `?.`, plus
  `Mode=OneWay|TwoWay|OneTime` (OneTime is treated as OneWay), `BindBack=`, `Fallback=`
  (`yui_vm.rs` `parse_binding`).
- Commands: `command="{x:Bind m(args)}"`. In templates, the arguments are recovered at dispatch
  time by iterating `items` again up to the clicked index (`VmGen::command`): O(n), and wrong if
  `items` isn't deterministic (the risk in yog-kit.md §15).
- `ViewModel` hooks: `on_open`, `on_close`, `on_back`, `customize`.
- Components (`yui_component.rs`): built once with string `Props`. In VM screens an instance is
  **rebuilt from scratch** whenever one of its bound properties changes
  (`VmGen::component_props`).

What's missing, compared with what the user asked for: can-execute, gestures, async commands,
change notification and dependency tracking, computed memoization, observable collections,
async loading state, most binding features (modes, formats, converters, element/ancestor
references), validation, data triggers, template selectors, navigation and dialogs (P10),
messaging, lifecycle beyond open/close/back, server sync, and designer support for any of these.

---

## 2. Principles

1. **The UI is a function of the VM.** Authors never think about notifications. The framework
   decides *when* to re-evaluate. Correctness never depends on the tracker: the `Slot` diff stays,
   and anything the tracker can't see falls back to "everything is dirty".
2. **Markup holds the wiring, Rust holds data and logic.** No widget access from the VM. Gestures,
   templates and styles are in markup. Commands, validation and messages are in Rust.
3. **One proc-macro attribute, codegen for the rest.** `#[view_model]` (in `yog-macros`,
   re-exported as `yog_kit::view_model`) only describes the VM: fields, masks, commands, handlers.
   Everything view-specific is generated from `.yui` by yog-cli. Neither side needs to parse the
   other's source.
4. **Generic over named.** `#[on(E)]` for every event, like D2. `Codec` for every boundary, like D1.
5. **Styling is someone else's job.** MVVM sets flags (`DISABLED`, `INVALID`, `SELECTED`, …) and
   toggles styles. Materials decide what that looks like.

---

## 3. The view model: `#[view_model]`

### 3.0 What a mod author writes (the boilerplate budget)

The design target, from the user: **a mod's code is its logic and nothing else.** For UI that
means:

| The author writes | Generated or declared instead |
|---|---|
| The VM struct: fields, with `#[view_model]` | the `ViewModel` impl, change notification, field masks, `Default`-based construction on open |
| Command bodies (`fn save(&mut self)`), only when there's logic | command wiring, can-execute → `enabled`, gestures, item-command arguments, running/error state |
| Rules that are real logic (`#[validate(name)] fn unique(..)`) | built-in rules from attributes, error storage, error display, `is_valid()` |
| Server logic that decides what the state *is* | client/server sync of that state (`Synced<T>`, §12.4), command transport (C2S), menu bookkeeping |
| Drawing code of a custom-drawn control | surface registration, the static `Mutex` session, input routing into it (§12.8) |
| — | screen registration, `register(registry)` calls, menu entries, opening a screen from a server packet (§12.5), persisted UI state, settings screens (§12.6), container menus (§12.7) |

What disappears entirely: getters that only expose a field (bind private fields directly, F12),
one-line methods that only combine fields (`&&`, `||`, comparisons, `?.`, `??` in bindings, §5.2),
`impl ViewModel`, `fn register`, `handle()` / `update()` plumbing, `static Mutex<Session>`
singletons, `Once` registrations, packet encode/decode (D1 Codec), and hand-written
`on_client_packet(CH, |ev| decode(ev.payload) …)` glue.

**The smallest screens:**

- A screen with no state at all (an info or credits page, a static menu with `cx.navigate` or
  `cx.close()` buttons) has **no Rust**. The `.yui` alone compiles to a `Vm = ()` screen (§8).
- A settings screen for a config struct has **no `.yui` and no VM**. It's generated from the struct
  (§12.6).
- A form or list screen is **the struct plus the commands that do something**:

  ```rust
  #[view_model]
  #[derive(Default)]
  pub struct RenameVm {
      #[validate(non_empty, max_len = 32)] name: String,
      #[persist] recent: Vec<String>,
  }

  #[view_model]
  impl RenameVm {
      #[command(can = self.is_valid())]
      fn save(&mut self, cx: &mut Cx<Self>) { protocol::RENAME.send(&self.name); cx.close(); }
  }
  ```

  The markup supplies the rest: `<TextBox text="{x:Bind name, Mode=TwoWay}"/>`,
  `<ErrorText for="#name"/>`, `<ListView items="{x:Bind recent}" activate="{x:Bind pick(item)}"/>`,
  `<Button command="{x:Bind save()}" gesture="Enter"/>`, and `menu_entry=` or `open_on=` on the
  root.

yog-cli lints the budget (`yog check`, warnings that the IDE can quick-fix): a VM method whose body
is just `self.field` or a field expression ("bind `field` directly"), a hand-written
`impl ViewModel`, `fn register` calling only generated registrations, or a `static Mutex` touched
only from a Surface and a VM (§12.8).

### 3.1 Authoring

```rust
use yog_kit::prelude::*;          // view_model, command, computed, on, recv, validate, persist, Cx, ObsVec, …

#[view_model]
#[derive(Default)]
pub struct ModListVm {
    mods: Vec<ModEntry>,
    #[persist] filter: Filter,           // saved between sessions (Codec), §12.3
    search: String,
    selected: Option<String>,
}

#[view_model]
impl ModListVm {
    #[on(Opened)]
    fn load(&mut self) {
        if self.mods.is_empty() { self.mods = load_mods(); }
    }

    #[computed]
    fn visible_ix(&self) -> Vec<u32> {             // memoized; callers get &Vec<u32>
        let q = self.search.trim().to_lowercase();
        (0..self.mods.len() as u32).filter(|&i| {
            let m = &self.mods[i as usize];
            self.filter.allows(m) && m.matches(&q)
        }).collect()
    }

    fn visible(&self) -> impl Iterator<Item = &ModEntry> {
        self.visible_ix().iter().map(|&i| &self.mods[i as usize])
    }

    #[command(can = self.has(filter))]
    fn set_filter(&mut self, filter: Filter) { self.filter = filter; }
}
```

Rules:

- `#[view_model]` on the **struct**:
  - assigns each field a bit (`Mask`, a `u64`; fields past the 63rd share one overflow bit, so
    they are tracked coarsely, and the macro warns suggesting a sub-struct; capability rule: no
    field limit);
  - adds **no fields**: the struct stays exactly as written, so plain struct literals work
    (Q2). The runtime state (memo cache, validation errors, the frozen flag) lives in a
    `VmCore` owned by the host next to the VM (`VmCell<V> { vm: V, core: VmCore }`). While the
    host runs a binding pass, a command, a handler or a validation pass, it installs the core as
    the *current core* (a scoped thread-local pointer, one read per access). Generated code
    (`#[computed]`, `errors()`, `is_valid()`) reaches it through `yog_kit::core::current()`.
    Outside the host (a VM built in a plain unit test) there is no current core: computed values
    are recomputed and validation rules run directly on the fields, which gives the same results
    without caching;
  - implements `ViewModel`.
- `#[view_model]` on **each inherent `impl` block** of the VM: every method is analysed (§3.2)
  and helper attributes are collected: `#[command]`, `#[computed]`, `#[on]`, `#[recv]`,
  `#[validate]`, `#[design]`, `#[volatile]`. A VM method used from markup should live in an
  annotated block. A method the schema check (§5.9) doesn't find there (a trait method, a
  method from another crate's impl, an unannotated block) still binds: codegen treats it as
  `#[volatile]` (read mask `ALL`, and `ALL` written when it's a `&mut self` command) and
  `yog check` warns (capability rule).
- `impl ViewModel for X {}` is **deleted** from user code, along with the `on_open` / `on_close` /
  `on_back` / `customize` trait methods (replaced by `#[on]`, §10).
- Item types (`ModEntry`, `IotaView`, …) need nothing. They are plain data.

### 3.2 Change tracking

The macro computes two masks per method from its token stream:

- **R(m), the read set**: every field `f` that appears as `self.f`, plus `R(n)` for every
  `self.n(…)` call. Calls are resolved across annotated impl blocks through associated consts,
  so the closure is computed by rustc's const evaluation.
- **W(m), the write set**, for `&mut self` methods: every field mentioned as `self.f` (mentioned,
  not just assigned: this over-approximates, which is safe), plus `W(n)` for called `&mut self`
  methods.
- **Escapes make the set `ALL`**: `self` used other than as `self.ident` (`helper(self)`,
  `let s = &mut *self`, `self` passed into a closure that outlives the expression), or a call to
  `self.n()` where `n` isn't in an annotated block. Macro invocations are scanned as token
  streams, so `format!("{}", self.x)` counts.
- `#[volatile]` on a method sets its R to `ALL`. Use it for methods that read state outside the
  VM: globals, `Cell`/`Mutex` fields, time.

Generated (per VM):

```rust
impl ModListVm {
    #[doc(hidden)] pub const __F_MODS: Mask = Mask::bit(0);
    #[doc(hidden)] pub const __F_FILTER: Mask = Mask::bit(1);
    #[doc(hidden)] pub const __F_SEARCH: Mask = Mask::bit(2);
    #[doc(hidden)] pub const __F_SELECTED: Mask = Mask::bit(3);
    #[doc(hidden)] pub const __R_visible_ix: Mask = Self::__F_MODS.or(Self::__F_FILTER).or(Self::__F_SEARCH);
    #[doc(hidden)] pub const __R_visible: Mask = Self::__R_visible_ix.or(Self::__F_MODS);
    #[doc(hidden)] pub const __W_set_filter: Mask = Self::__F_FILTER;
    #[doc(hidden)] pub const __W_load: Mask = Self::__F_MODS;
}
```

**The host's algorithm** (replaces `dirty: bool` in `host.rs`):

```
dirty: Mask                                   // ALL on open, language change, resize-only = 0
after dispatch(ev)        → dirty |= mask returned by View::dispatch (the command's W, or the
                            field of a two-way write, or ALL for cx calls that may write)
after #[on]/#[recv] handler → dirty |= W(handler)
after handle.update(closure), Vm::with in tasks, TestHost::vm_mut → dirty = ALL (opaque)
ObsVec fields              → their own change logs (§3.4), also mark their field bit
frame: if dirty != 0 { core.invalidate_memos(dirty); core.freeze(); view.update(.., dirty); core.thaw(); dirty = 0 }
```

In `View::update`, every binding is guarded by its read mask, which codegen builds from the
binding expression: field paths give their root field's bit, method calls give `R(method)`, and
item-template bindings give the mask of their `vm.` references plus the items expression's mask:

```rust
// @yui mod_list.yui:27 text="{x:Bind count_text()}"
if d.any(<ModListVm>::__R_count_text) {
    if self.b0.set(::yog_kit::bind::text(&vm.count_text())) { … }
}
```

`Strings::x()` / `@res:Strings.x` texts are re-evaluated on `LanguageChanged` (a full pass). Free
functions in bindings (`percent(v)`) are pure by contract.

**Verification.** In debug builds the host also runs a *shadow pass* every N frames: every
skipped binding is evaluated into a scratch `Slot` comparison. If one would have changed, the
host logs once per binding:

```
[yog-kit] mod_list.yui:27 text="{x:Bind count_text()}" changed but wasn't scheduled:
          count_text reads state the tracker can't see (a global, Cell, or a method outside #[view_model]).
          Mark it #[volatile] or move that state into a field.
```

`TestHost` runs the shadow pass after every input and **fails the test** on a mismatch. A mod's
ordinary UI tests therefore also prove that its dependencies are right.

**Why not signals.** We considered and rejected `Prop<T>` / `Signal<T>` fields with runtime
read-tracking (Leptos/MobX-style). They would change every field's type, every read (`.get()`)
and every write (`.set()`), and make VMs awkward to unit test and serialize. Static masks give the
same skip-work benefit with plain fields. The cost is conservatism: a command that mentions a
field re-evaluates its bindings even if it didn't change it, and the `Slot` diff then makes that
free at the widget level.

### 3.3 Computed properties

`#[computed] fn x(&self) -> T` (with `T: Send + 'static`) is rewritten to return `&T`:

- During a binding pass (VM frozen), the first call computes and caches the value in the host's
  `VmCore` (the current core, §3.1).
  Later calls in the same pass and later passes return the cache until `dirty ∩ R(x) ≠ ∅`.
- Outside a pass (inside commands, handlers or tests) the value is always recomputed into a
  per-call scratch arena that the host clears after the call. A computed value can't be stale
  inside a command that has just written its inputs.
- A computed method is still an ordinary method for tracking: `R(caller) ⊇ R(x)`.

Modlist calls `visible()` from `items`, `count_text`, `has(…)` ×2, `selected`, `selected_id` and
the `can` of three filter commands: about 9 filters over all mods per pass in v1, and 1 with
`#[computed]`.

### 3.4 Observable collections: `ObsVec<T>`

```rust
pub struct ObsVec<T> { /* Vec<T> + change log + epoch */ }
impl<T> Deref for ObsVec<T> { type Target = [T]; }            // reads are slice reads
impl<T> ObsVec<T> {
    pub fn push(&mut self, v: T);
    pub fn insert(&mut self, at: usize, v: T);
    pub fn remove(&mut self, at: usize) -> T;
    pub fn move_item(&mut self, from: usize, to: usize);
    pub fn set(&mut self, at: usize, v: T);
    pub fn update(&mut self, at: usize, f: impl FnOnce(&mut T));   // marks one row
    pub fn retain(&mut self, f: impl FnMut(&T) -> bool);           // emits removes
    pub fn clear(&mut self);
    pub fn replace(&mut self, v: Vec<T>);                          // Reset
    pub fn sync_by_key<K: Eq + Hash>(&mut self, new: Vec<T>, key: impl Fn(&T) -> K)
        where T: PartialEq;                                        // minimal insert/remove/move/update diff
    pub fn iter_mut_tracked(&mut self) -> impl Iterator<Item = Tracked<'_, T>>; // DerefMut marks the row
}
pub enum Change { Insert { at: u32, len: u32 }, Remove { at: u32, len: u32 }, Move { from: u32, to: u32 }, Update { at: u32 }, Reset }
```

- `items="{x:Bind patterns}"` on an `ObsVec` field: codegen picks `ItemsHost::apply_changes`
  through autoref specialization on the expression type, so the markup doesn't change. Rows are
  inserted, removed and moved without rehashing keys, and `Update { at }` re-evaluates that row's
  bindings only. For virtualized lists, changes outside the window only shift the spacers.
- Row identity follows the log. `key=` becomes optional for `ObsVec` and is used only after a
  `Reset` (reconciliation, as today).
- Item commands and selection map the row back by position, which is O(1) (§4.3).
- The log is bounded. If the host missed more than the bound (for example, a screen closed during
  heavy updates), it sees `Reset`.
- A plain `Vec<T>`, `&[T]`, or any iterator works as in v1: keyed reconciliation over the whole
  list when its mask is dirty.
- **Filtered views** (`visible()` over an `ObsVec`) are iterators, hence keyed reconciliation. An
  incremental `CollectionView` (filter + sort over an `ObsVec`, emitting its own diffs) is
  deferred (Q8).

**Selection.** `selected=` stays as in v1 (`Option<K>`, `BindBack`/TwoWay).
`selection_mode="multiple"` binds a `BTreeSet<K>` field (Ctrl/Shift-click, Ctrl+A).

### 3.5 Batching and transactions

Nothing is applied before the frame: any number of writes inside one command, handler, task step
or queued update coalesce into one `dirty` mask and one pass. `ObsVec` logs coalesce too (insert
then remove of the same row cancels out). An explicit transaction API is therefore unnecessary.
The one place it matters is server sync (§12.4): a server tick's changes are applied as one
message.

### 3.6 Async loading state: `Loadable<T>`

```rust
pub enum Loadable<T, E = Text> { Idle, Loading { progress: Option<f32> }, Ready(T), Failed(E) }
impl<T, E> Loadable<T, E> {
    pub fn is_loading(&self) -> bool; pub fn ready(&self) -> Option<&T>; pub fn error(&self) -> Option<&E>; pub fn progress(&self) -> Option<f32>;
}
```

`ContentPresenter` gains `Loading` and `Failed` template slots next to `Empty`:

```xml
<ContentPresenter content="{x:Bind details}">          <!-- details: Loadable<Details> -->
    <DataTemplate x:DataType="Details"> … </DataTemplate>
    <ContentPresenter.Loading><Label text="@res:Strings.loading"/></ContentPresenter.Loading>
    <ContentPresenter.Failed><Label text="{x:Bind error}" color="@res:Theme.Error"/></ContentPresenter.Failed>
</ContentPresenter>
```

`Loadable` is filled by async commands or tasks (§4.5).

---

## 4. Commands

### 4.1 Declaration

Any VM method callable from markup is a command, as in v1. `#[command]` is needed only for
options:

```rust
#[command(can = !self.stack.is_empty())]          // expression over &self (and the parameters)
fn clear(&mut self) { protocol::RESET.send(&()); }

#[command(can = can_remove)]                       // or a method taking the parameters by reference
fn remove(&mut self, id: String) { … }
fn can_remove(&self, id: &String) -> bool { … }

#[command]                                         // async: §4.5
async fn reload(vm: Vm<Self>) -> Result<(), Text> { … }
```

For each method the macro generates `__can_<name>(&self, params…) -> bool`, which returns `true`
without `can =`, and its read mask `__R_can_<name>`.

### 4.2 Can-execute → `enabled`

For every element with `command=`:

```
effective enabled = (explicit enabled= binding, default true) && __can_<cmd>(args) && !running(cmd)
```

This is evaluated as a binding with mask `R(enabled) | R(can) | R(args)`. The host's `activate`
already ignores disabled widgets, and the widget gets the `DISABLED` state that materials draw.
Gestures and `TestHost::exec` check the same condition.

### 4.3 Parameters (`CommandParameter`)

Arguments stay inline, which is the compiled-binding equivalent of `CommandParameter` and
type-checked by rustc:

```xml
<Button command="{x:Bind set_filter(Filter::Yog)}"/>
<Button command="{x:Bind vm.remove(id)}"/>                <!-- inside an item template -->
<TextBox command="{x:Bind search_now(#search.text)}"/>    <!-- element reference, §5.6 -->
```

**Item commands, v2.** At update time each row evaluates the commands' arguments into the row
instance:

```rust
inst.c0.set((Clone::clone(&item.id),))
```

`dispatch` reads `self.l1.row(index).c0` and doesn't iterate `items` again: O(1), and correct
for non-deterministic `items`. Arguments must be `Clone + PartialEq` (they're stored in a
`Slot`). This replaces the `__args = items.enumerate().nth(index)` code in `VmGen::command`.

`command_parameter=` exists only on **component** instances, which expose a `Command<P>`
property (§9): `<RowActions delete="{x:Bind vm.remove}" command_parameter="{x:Bind id}"/>`.

### 4.4 Gestures and input bindings

```xml
<Screen … x:ViewModel="ModListVm">
    <Screen.InputBindings>
        <KeyBinding gesture="Ctrl+F" command="{x:Bind cx.focus(ids::SEARCH)}"/>
        <KeyBinding gesture="Delete" command="{x:Bind remove_selected()}"/>
    </Screen.InputBindings>
    <TextBox x:Name="search" text="{x:Bind search, Mode=TwoWay}">
        <TextBox.InputBindings>                              <!-- only while the box is focused -->
            <KeyBinding gesture="Down" command="{x:Bind select_next()}"/>
            <KeyBinding gesture="Up" command="{x:Bind select_prev()}"/>
        </TextBox.InputBindings>
    </TextBox>
    <Button text="@res:Strings.save" gesture="Ctrl+S" command="{x:Bind save()}"/>   <!-- same as a KeyBinding to its command -->
    <ListView items="{x:Bind visible()}" activate="{x:Bind open(id)}"/>          <!-- double-click or Enter on a row -->
</Screen>
```

- Gesture strings (`Ctrl+Shift+S`, `F5`, `Delete`, `Alt+Enter`) are parsed at codegen into
  `(key, mods)` constants from `yog_ui::event::keys`. An unknown key is a build error with the
  list of names.
- **Routing**, in this order: the focused element's `InputBindings` (and its ancestors'), then the
  focused text box for printable and editing keys, then the screen's `InputBindings`, then the
  built-in navigation (Tab, Enter/Space, arrows in lists, Esc). While a text box is focused, plain
  keys without modifiers go to the box. Gestures with Ctrl/Alt or function keys pass through.
- A gesture whose command can't execute is not consumed, so it falls through to the game.
- `activate=` is the row's default action (double-click, Enter while the list is focused), using
  the row's captured arguments like item commands.
- Context menus (right-click) are deferred. They need popups, which come with §11's overlay layer.

### 4.5 Async commands and tasks

yog-kit gets a tiny single-threaded executor polled on the render thread each frame (and when a
waker fires). There is no tokio. Background work goes to a small thread pool:

```rust
#[command]
async fn reload(vm: Vm<Self>) -> Result<(), Text> {
    vm.with(|s| s.details = Loadable::Loading { progress: None });
    let mods = yog_kit::background(load_mods).await;            // runs off the render thread
    vm.with(|s| s.mods.replace(mods));
    Ok(())
}
```

- `Vm<Self>` is the task-side handle. `with(|&mut Self| R) -> R` borrows the VM synchronously
  during the poll. The host never polls while it holds the VM, so this can't deadlock or alias.
  It also offers `read`, and `cx()` for navigation and dialogs. A `with` closure is opaque, so it
  marks `ALL` dirty (as v1 does today). That's fine at task granularity.
- Command state per async command: `{x:Bind cmd.reload.running}`, `cmd.reload.error`
  (`Option<&Text>`), `cmd.reload.progress`. `cmd` is a reserved binding root like `cx` and `vm`.
  While running, the command can't execute (§4.2), so double clicks don't run it twice.
- `Err(e)` stores the error in the command state and raises `#[on(CommandFailed)]`.
- Tasks are owned by the screen and **dropped when the screen closes**, unless declared with
  `#[command(detached)]`. They are also cancelled by `cx.cancel(Self::RELOAD)`.
- Other awaitables: `yog_kit::sleep(Duration)` (frame-clock based), `cx.dialog::<V>(…)` (§11), and
  `Rpc<Req, Resp>::call(&req)`, a request/response extension of F3 channels with a correlation
  id. It's proposed here for F3 and needs no extra ABI.
- Non-command tasks: `cx.spawn(async move |vm| …)` from any handler.

---

## 5. Bindings

### 5.1 Syntax

```
{x:Bind expr [| conv[(args)]]* [, Mode=OneWay|TwoWay|OneTime|OneWayToSource]
             [, Format='…'] [, BindBack=method] [, Delay=300ms] [, Trigger=Change|Submit|Blur]}
```

`Fallback=` is **deleted**. `a?.b ?? 'none'` covers both WPF FallbackValue (a path failed) and
TargetNullValue (the value is `None`), because in compiled bindings both are the same `None`.

### 5.2 Expressions

| Form | Example | Notes |
|---|---|---|
| paths, calls, `Type::f()` | `state.name`, `has(Filter::Yog)` | v1 |
| `?.` and `??` | `status?.text ?? ''` | `??` unwraps `Option`; the right side may be any expression |
| `!`, `&&`, `\|\|` | `!busy && has_selection()` | new (v1 refused them; authors kept writing one-line methods) |
| `==`, `!=`, `<`, `<=`, `>`, `>=` | `count > 0` | new |
| indexers | `stack[0]?.value` | `[i]` compiles to `.get(i)`, giving an `Option` |
| literals | `1`, `1.5`, `true`, `'text'` | v1 |
| roots | `vm`, `item`, `index`, `parent`, `cx` (commands only), `cmd`, `errors`, `#name` | §5.6–5.7 |

Arithmetic stays out. Use `Format=` or a method.

### 5.3 Converters and formats

- **Pipes** call functions in scope (the `.g.rs` is `include!`d into the code-behind module, so
  `use` statements there apply): `{x:Bind progress | percent}` compiles to `percent(&progress)`,
  and `{x:Bind size | bytes(1)}` to `bytes(&size, 1)`. Pipes chain left to right.
- **Two-way converters** are a trait. The built-in `Parse` uses `FromStr` + `Display`:

  ```rust
  pub trait Converter<V> { type Target; fn to(&self, v: &V) -> Self::Target;
                           fn back(&self, t: &Self::Target) -> Result<V, Text>; }
  ```

  `text="{x:Bind amount | Parse, Mode=TwoWay}"` works for any `FromStr` field. Without a
  converter, a TwoWay `text` on a non-`String` field picks `Parse` automatically. A `back` error
  doesn't write the field. It becomes a validation error on it (§6).
- **`Format=`** is a Rust format spec checked by rustc: `text="{x:Bind ratio, Format='{:.0}%'}"`
  compiles to `format!("{:.0}%", ratio)`. For localized text, use resources:
  `{x:Bind Strings::count(n)}`.
- **`yog_api::Text`** (F6) binds to `text` directly (`BindText for Text` renders it), so
  `{x:Bind mishap.text()}` works with `#[derive(Tr)]`.

### 5.4 Modes

| Mode | Semantics | Codegen |
|---|---|---|
| `OneWay` (default) | VM → widget when the mask is dirty | v1 |
| `OneTime` | evaluated on open (and on language change), never again | the binding's mask is `0`; it runs in the open pass |
| `TwoWay` | plus widget → VM on input | write to a place path or `BindBack=`; the dirty mask is the place's root field bit |
| `OneWayToSource` | widget → VM only; the VM never pushes | e.g. `TextBox text="{x:Bind draft, Mode=OneWayToSource}"`, `ScrollView offset=` |

TwoWay targets: `TextBox.text`, `ToggleButton/CheckBox.checked`, `Slider.value`,
`TabBar.selected_tab`, `ListView.selected`, `ScrollView.offset`. The new controls come from the
materials doc; this doc only gives them two-way plumbing through one `ViewEvent::ValueChanged
{ part, value }` instead of a variant per control.

`Delay=300ms` debounces the write-back, which suits a search box filtering a large list.
`Trigger=Submit|Blur` writes on Enter or on focus loss instead of on every edit.

### 5.5 Nested paths and optional chains

Unchanged from v1 plus indexers. Note that **binding private fields** works because the `.g.rs`
is included into the VM's module (F12). F12 recommends binding fields directly instead of
writing trivial getters, and v2 keeps that.

### 5.6 Element-to-element bindings (`ElementName`)

`#name.prop` reads another element's **view state**, meaning what the host owns, not what the VM
owns:

| Element | Properties |
|---|---|
| any | `hovered`, `pressed`, `focused`, `visible`, `enabled` |
| TextBox | `text`, `caret`, `is_empty` |
| ListView | `selected_index`, `len` |
| ScrollView/ListView | `offset`, `at_end` |
| CheckBox/ToggleButton/Slider/TabBar | `checked`, `value`, `selected_tab` |

```xml
<Label text="{x:Bind #search.text.len(), Format='{}/64'}"/>
<Button command="{x:Bind go(#search.text)}" enabled="{x:Bind !#search.is_empty}"/>
<Label visible="{x:Bind #this.hovered}"/>     <!-- RelativeSource Self -->
```

Codegen resolves `#name` to the host part (text box slot, list owner, scroller) at build time,
so an unknown name is a build error. Element-state bindings carry a reserved `VIEW` bit in their
mask, which the host sets whenever any part state changes.

### 5.7 Scopes: `data_context`, ancestors, nested templates

- `data_context="{x:Bind settings}"` on any container rebinds bare names in its subtree to that
  value. `vm.` still reaches the root. Commands in the subtree call `vm.settings.method()`. For
  TwoWay inside it, `data_context` must be a place path. If the value is an `Option`, the subtree
  is collapsed while it is `None`, like a `ContentPresenter` without templates.
- **Nested lists** (a list inside an item template) become supported. Scope roots are `item` (the
  current row), `parent` (the enclosing row; `parent.parent` and so on), and `vm` (the screen).
  This is RelativeSource `FindAncestor` for data. For element state, `#name` works across
  template boundaries for screen-level elements only.
- `index` is the row index, as in v1.

### 5.8 Compile-time checking

Three layers, all at `yog build` / `yog check`:

1. **Markup structure** (yog-cli, exists): unknown elements and attributes, non-bindable
   attributes, unknown style or resource keys. The bindable-attribute table moves into one data
   file shared with IDLE (`yui-schema.json`, generated from yog-cli), so the designer and the
   compiler agree.
2. **Names** (new, before rustc): `yog ui schema` (syn over the code-behind, planned in
   yog-kit.md §11) gives fields, methods with arity and return types, `#[command]`s and
   `#[computed]`s. Every binding root is checked against it, with suggestions:

   ```
   error: src/mod_list.yui:41:31: no field or method `serch` on ModListVm (did you mean `search`?)
      41 |  text="{x:Bind serch, Mode=TwoWay}"
         |                ^^^^^
   ```

   Item scopes use `x:DataType`, which becomes **required** for templates (the IDE needs it
   anyway). When the code-behind can't be resolved (types from another crate), this layer is
   skipped and layer 3 still catches the error.
3. **Types** (rustc): the generated `// @yui file:line:col` comments stay, and `yog build` now
   rewrites cargo's JSON diagnostics whose span is in a `.g.rs` to the `.yui` position, with the
   original attribute as the excerpt. `bind.rs`'s `#[diagnostic::on_unimplemented]` messages are
   kept.

### 5.9 Generated code, before and after

v1 (today, `mod_list.g.rs`, abbreviated):

```rust
fn update(&mut self, root: &mut Widget, vm: &ModListVm, u: &mut Update) {
    if self.b0.set(::yog_kit::bind::text(&(vm.count_text()))) { root.at_path_mut(&[0, 2]).set_text(…); u.relayout(); }
    … every binding, every time …
}
fn dispatch(&mut self, vm: &mut ModListVm, cx: &mut Cx<'_, ModListVm>, ev: ViewEvent) {
    match ev { ViewEvent::Action(Action { owner: 0, slot: 1, .. }) => { vm.set_filter(Filter::Yog); } … }
}
```

v2:

```rust
const _: () = ::yog_kit::__codegen::require(2);
type __Vm = ModListVm;

fn update(&mut self, root: &mut Widget, vm: &__Vm, u: &mut Update) {
    let d = u.dirty();
    // @yui mod_list.yui:27:20 text="{x:Bind count_text()}"
    if d.any(__Vm::__R_count_text) {
        if self.b0.set(::yog_kit::bind::text(&vm.count_text())) { root.at_path_mut(&[0, 2]).set_text(…); u.relayout(); }
    }
    // @yui mod_list.yui:52:21 command="{x:Bind set_filter(Filter::Yog)}" (can-execute)
    if d.any(__Vm::__R_can_set_filter) {
        let on = vm.__can_set_filter(&Filter::Yog);
        if self.b5.set(on) { ::yog_kit::bind::set_enabled(root.at_path_mut(&[1, 1, 1]), on); u.repaint(); }
    }
    …
}

fn dispatch(&mut self, vm: &mut __Vm, cx: &mut Cx<'_, __Vm>, ev: ViewEvent) -> Mask {
    match ev {
        ViewEvent::Action(Action { owner: 0, slot: 1, .. }) if vm.__can_set_filter(&Filter::Yog) => {
            vm.set_filter(Filter::Yog); __Vm::__W_set_filter
        }
        ViewEvent::TextChanged { slot: 0, text } => { vm.search = text; __Vm::__F_SEARCH }
        ViewEvent::Action(Action { owner: 1, slot: SELECT, index }) | ViewEvent::Select { list: 1, index } => {
            let (k,) = Clone::clone(self.l1.row(index).key_args()); vm.select(k); __Vm::__W_select
        }
        ViewEvent::Gesture(0) if vm.__can_remove_selected() => { vm.remove_selected(); __Vm::__W_remove_selected }
        _ => Mask::NONE,
    }
}
```

### 5.10 yui_vm.rs restructuring

`yui_vm.rs` (1830 lines) splits into `yui/` modules: `expr.rs` (lexer and parser; the new
operators), `binding.rs` (options, modes, converters, formats), `scope.rs` (vm, item, parent,
data_context, `#name`, cmd, errors), `gen_view.rs` (update and dispatch with masks), `gen_list.rs`
(ItemsHost, ObsVec, template selectors, nested lists), `gen_control.rs` (components, §9),
`gen_input.rs` (gestures), `schema.rs` (`yog ui schema` + name checks), `diag.rs` (JSON
diagnostic remapping). `yui.rs` keeps only XML parsing and the static tree emitter (`Gen`,
`static_widget`). `CODEGEN` becomes 2, and yog-kit's `require` rejects 1.

---

## 6. Validation

```rust
#[view_model]
#[derive(Default)]
pub struct WaypointVm {
    #[validate(non_empty, max_len = 32)] name: String,
    #[validate(range = -30_000_000..=30_000_000)] x: i32,
    …
}

#[view_model]
impl WaypointVm {
    #[validate(name)]                                   // a custom rule for a field
    fn name_unique(&self) -> Result<(), Text> {
        if self.taken.contains(&self.name) { Err(Strings::waypoint_taken().into()) } else { Ok(()) }
    }

    #[command(can = self.is_valid())]                   // is_valid() is generated
    fn save(&mut self, cx: &mut Cx<Self>) { …; cx.close(); }
}
```

- Built-in rules: `non_empty`, `min_len`, `max_len`, `range`, `matches = "regex"` (behind a
  feature), and `custom = path`. Messages are `Text`, from yog-kit's built-in strings, which
  follow the game language.
- Errors live in the host's `VmCore` (the current core, §3.1), keyed by field bit. `self.errors()`
  and `self.is_valid()` read them from there; without a host they evaluate the rules directly. A field's rules re-run when the field's bit is
  dirty or when a custom rule's read mask is. Conversion errors (§5.3) are stored under the field
  too.
- **Touched semantics**: errors are reported for a field after its first edit, or after
  `cx.validate_all()` (for example at the start of `save`). `is_valid()` always reflects all
  rules.
- **In the view**:
  - Any element with a TwoWay binding to an invalid field gets the new `INVALID` state flag
    (materials draw it, §7) and, unless it has its own `tooltip`, the error as its tooltip
    (`tooltip` comes from the materials doc).
  - `{x:Bind errors.name}` is an `Option<&Text>`. `{x:Bind errors.any}` is a bool.
  - `<ErrorText for="#name"/>` is a Label bound to the errors of whatever field element `name` is
    two-way bound to, collapsed when there's none.
- `TestHost`: `h.errors("name")`, `h.is_invalid(ids::NAME)`.

---

## 7. Styles, triggers and visual states (the hooks only)

MVVM's side of styling is limited to three hooks, and the materials doc owns what they look like:

1. **Interaction states**: `HOVER`, `PRESSED`, `FOCUSED`, `SELECTED`, `CHECKED`, `DISABLED`, set
   by the host as today. **New: `INVALID` (bit 6)** for validation. It still fits the `u8` state
   that `draw2d_material` takes, so the material ABI doesn't change. The `.ymat` `states` grammar
   needs to accept `invalid` (to be agreed with materials-and-vanilla-ui.md §2.2; see Q10).
   `disabled` now also comes from can-execute (§4.2).
2. **Conditional styles** (data triggers): `style:<Key>="{x:Bind cond}"` applies resource style
   `Key` on top of the element's own while `cond` is true, like toggling a CSS class:

   ```xml
   <Label style="@Detail" style:Danger="{x:Bind status?.mishap ?? false}" text="{x:Bind status?.text ?? ''}"/>
   ```

   Codegen knows `Danger`'s attributes. If they are paint-only, toggling is a repaint. Otherwise
   it's a relayout. There are no new state bits, no trigger XML, and the IDE previews each style
   with a checkbox.
3. **Bindable paint attributes**, as today (`bg="{x:Bind accent}"`). Under the materials doc, `bg`
   takes a `Brush`, so a binding may return a material handle.

Animations and visual transitions are out of scope.

### 7.1 Data template selectors: `match`

A template selector is a Rust `match` on the item. Patterns are written in the template, and
the first one that matches wins:

```xml
<ListView items="{x:Bind patterns}" item_height="12">
    <DataTemplate x:DataType="PatternView" match="PatternView { pending: true, .. }">
        <PatternEntry name="@res:Strings.pattern.pending" signature="{x:Bind signature}" style:Pending="{x:Bind true}"/>
    </DataTemplate>
    <DataTemplate x:DataType="PatternView">
        <PatternEntry name="{x:Bind name}" signature="{x:Bind signature}"/>
    </DataTemplate>
</ListView>

<ItemsControl items="{x:Bind stack}">
    <DataTemplate x:DataType="Iota" match="Iota::Vec3(x, y, z)">      <!-- pattern bindings are in scope -->
        <Label text="{x:Bind Strings::vec3(x, y, z)}"/>
    </DataTemplate>
    <DataTemplate x:DataType="Iota" match="Iota::List(items)">
        <ItemsControl items="{x:Bind items}"> … nested, `parent` reaches the outer iota … </ItemsControl>
    </DataTemplate>
    <DataTemplate x:DataType="Iota"><Label text="{x:Bind tr()}"/></DataTemplate>
</ItemsControl>
```

- Codegen emits `match item { <pat> => 0, <pat> => 1, _ => last }`. The row instance is an enum
  of template instances. When a row's template changes, that row is rebuilt.
- The match must be exhaustive. rustc checks this, because without a catch-all template codegen
  emits no `_` arm.
- `ContentPresenter` accepts the same, which covers "a different view per enum variant" (WPF
  implicit DataTemplates by type).

---

## 8. Removing the old `.yui` path

| Delete | Replacement |
|---|---|
| Non-VM `<Screen>` codegen in `yui.rs::generate_for`: `root_with`, `root`, `render`, `render_with`, `click`, `with_last`, `text_box`, `mod events`, `__YUI_LAST` | A `<Screen>` without `x:ViewModel` compiles through the VM generator with `type Vm = ()` (a static screen still gets `register`, `open`, `ids` and `TestHost`). `build()` (the static tree as a `Widget`) and `ids` stay generated for every `.yui`, for code that drives the tree through the low-level `yog_api::ui` path (below) |
| `on_click` / `on_change` / `on_submit` attributes in `.yui` | `command=`, TwoWay, `Trigger=Submit` |
| Component `<Event>` declarations and `on_click="{x:Bind on_select}"` | `Command<P>` properties (§9) |
| `VmGen::component_props` (rebuild the instance when a property changes) | compiled controls patched in place (§9) |
| `ViewModel::{on_open, on_close, on_back, customize}` | `#[on(Opened|Closed|Back|Customize)]` (§10) |
| `Fallback=` | `??` |
| IDLE `designer/codebehind.rs` event-handler generation (`events::X => x()` arms) | command stubs in the `#[view_model] impl` (§14) |

`Widget::on_click` stays in yog-ui: yog-book uses it (`yog-book/src/lib.rs`, `renderer.rs`). Only
`.yui` loses it.

**Kept (capability rule):** the low-level UI path in yog-api/yog-ui is not part of this deletion.
A mod can still build or take a `Widget` tree (`build()` above, or by hand), lay it out with
`UiRoot`, draw it, receive raw UI events through `register_ui`, and hit-test or read rects
(`hit_test`, `find_rect`, `find_node`, `TextBoxState::handle`). What goes is the generated
global-state glue around it (`__YUI_LAST`, string `click()`), not the ability. `yog check` warns
when a `<Screen>` is driven this way instead of through the VM host.

---

## 9. Components v2: compiled controls

Components keep their file format and their role (WPF UserControls). Their implementation
changes:

```xml
<Component name="IotaRow" description="@res:Strings.component.iota_row" design_w="150" design_h="11">
    <Property name="kind" type="String" default="Number"/>
    <Property name="value" type="Text" default="1.00"/>
    <Property name="accent" type="Color" default="#FF5555FF" category="Appearance"/>
    <Property name="inspect" type="Command"/>                 <!-- replaces <Event> -->
    <Row w="100%" h="11" gap="4" align="center" command="{x:Bind inspect}">
        <Label text="{x:Bind kind}" shadow="false" no_wrap="true"/>
        <Label flex="1" text="{x:Bind value}" color="{x:Bind accent}" ellipsis="true"/>
    </Row>
</Component>
```

- `type=` takes Rust-facing type names: `String`, `Text`, `f32`, `i64`, `bool`, `Color`, `Brush`,
  `Units`, `Command`, `Command<P>`, `choice(a, b)`, or any path (`crate::Mode`). The designer maps
  them to editors. The old lowercase names (`text`, `number`, …) are removed.
- Generated: `pub struct IotaRow { pub kind: String, … }` (the props) and a compiled control with
  its own `Slot`s. The parent's generated code holds one instance per use and calls
  `inst.update(props, root_at_path, u, dirty)`. Only the changed parts of the instance are
  patched, so there is no rebuild.
- `Command` properties are filled by the parent with a command expression (`inspect="{x:Bind
  vm.inspect(id)}"`). The parent compiles the call into its own dispatch, and the component's
  body only routes clicks to "property `inspect`". `command_parameter=` on the instance supplies
  `P` for `Command<P>`.
- A component may declare `x:ViewModel="SpinnerVm"` for **internal state** (a spinner's text
  while editing, a color picker's hue). That VM is created per instance, has the same
  `#[view_model]` features, and its masks are separate from the parent's.
- The string `Props` path (`component::build(name, &[(k, v)])`, `from_props`, `PROPS`) stays
  for the designer host (`yog designer-lib`), so the Toolbox, Properties and the live canvas keep
  working, **and in game builds** for components chosen by name at runtime from data: ybook custom
  page types (ybook.md B5, page fields → properties), another mod's component, data-driven layouts
  (capability rule). `.yui` uses of a component never go through it; they use the compiled control.
- Inside item templates (an `IotaRow` per stack entry), each row holds a control instance, and rows are
  patched like any template binding.

Note: the materials doc deletes Modlist's `pill.yui` and `stat_bar.yui`. HexMod's components
(`IotaRow`, `PatternEntry`, `MishapBanner`) were deleted with its old casting screen (§15.2), so
no mod currently ships a component. `IotaRow` is used above only as an example.

---

## 10. Lifecycle and events: `#[on(E)]`

The same idea as D2's `registry.on::<E>()`, in declarative form on VM methods:

```rust
#[view_model]
impl CastingVm {
    #[on(Opened)]   fn start(&mut self, cx: &mut Cx<Self>) { … }
    #[on(Closed)]   fn stop(&mut self) { … }
    #[on(Back)]     fn back(&mut self) -> Flow { if self.editing { self.editing = false; Flow::Handled } else { Flow::Continue } }
    #[on(Tick)]     fn tick(&mut self, t: &Tick) { … }                  // client tick (20/s) while open
    #[on(PatternDrawn)] fn drawn(&mut self, m: &PatternDrawn) { … }    // a messenger message, §12.1
    #[recv(protocol::STACK)] fn apply(&mut self, report: Report) { … } // an F3 S2C channel, §12.2
}
```

Handler signatures are flexible. In order, a handler takes `&mut self` or `&self`, optionally the
event (`&E`, or `E` for channels), and optionally `&mut Cx<Self>`, which gives navigation,
dialogs, focus, `spawn` and `publish`. The return type is `()` or `Flow` (for `Back` and other
cancellable events, the `before::<E>()` analogue).

Built-in events (`yog_kit::events`):

| Event | When |
|---|---|
| `Opened` | the screen opened (every time). Load data here. |
| `Opened<P>` | the screen was opened by `open_on=` with payload `P` (§12.5). The handler takes `P` by value. |
| `Closed` | the screen closed, by any path |
| `Back` | Esc with nothing focused; `Flow::Handled` keeps the screen open |
| `Tick` | client tick while the screen is open (`dt`, tick count) |
| `Resized { w, h }` | GUI size changed |
| `LanguageChanged` | the game language changed (a full pass runs anyway) |
| `Returned<R>` | returned from a screen navigated to with `cx.navigate_for::<V, R>()` (§11) |
| `CommandFailed` | an async command returned `Err` (§4.5) |
| `Customize` | after each binding pass. Handler signature `fn(&self, root: &mut Widget)`, the escape hatch formerly `ViewModel::customize` |

The macro generates the `ViewModel` impl, which has one method,
`fn on_event(&mut self, cx, ev: &dyn Any) -> Option<Mask>`, dispatching by `TypeId` and returning
the handler's write mask. Subscriptions (messages, channels) are registered by the generated
`register()`, which self-registers per F8.

---

## 11. Navigation, dialogs and message boxes (P10)

### 11.1 Navigation between screens

```rust
cx.navigate::<details::View>(|vm| vm.show(id.clone()));            // typed; replaces navigate_to
cx.navigate_for::<picker::View, Option<u32>>(|vm| vm.color = c);  // the result comes back as Returned<Option<u32>>
cx.back();                                                          // in the picker: cx.back_with(Some(color))
cx.navigate_id("othermod:screen");                                  // another mod's screen, by UI id (no result)
```

The navigation stack stays per mod (`screen.rs` `NAV`), because Minecraft shows one `Screen` at a
time. `Returned<R>` is delivered to the VM that navigated, as an event (§10), so the result
doesn't require async.

### 11.2 Dialogs: overlays in the current screen

A dialog is a `.yui` with a `<Dialog>` root and a VM declared as a dialog:

```xml
<Dialog id="mymod:confirm_delete" width="200" height="80" x:ViewModel="ConfirmDeleteVm">
    <Column padding="8" gap="6">
        <Label w="100%" text="{x:Bind message}"/>
        <Row gap="4" halign="right">
            <Button text="@gui.cancel" command="{x:Bind cx.close_with(false)}" gesture="Escape"/>
            <Button text="@gui.yes" command="{x:Bind cx.close_with(true)}" gesture="Enter"/>
        </Row>
    </Column>
</Dialog>
```

```rust
#[view_model(dialog = bool)]                       // the result type
#[derive(Default)]
pub struct ConfirmDeleteVm { message: Text }

// Callback form, from any command:
cx.dialog::<confirm_delete::View>(|d| d.message = Strings::confirm_delete(&name).into())
  .then(|vm: &mut Self, yes: bool| if yes { vm.delete_selected() });

// Async form, in async commands and tasks:
if vm.cx().dialog::<confirm_delete::View>(init).await == Some(true) { … }   // None = dismissed (screen closed)
```

- The host keeps a **layer stack**. Layer 0 is the screen. Each dialog is a `Box<dyn Layer>`
  wrapping its own `Host<W>` (its own VM, tree, parts and focus) drawn over a scrim. Input goes to
  the top layer only, and focus is trapped inside it. Esc closes the dialog with the result
  type's `Default` (`false`, `None`).
- `.then(|vm, r|)` runs on the parent VM with a known write mask: the closure is opaque, so it's
  `ALL`. Prefer a method: `.then_call(Self::on_confirm)`, which has a known `W`.
- **Message boxes** ship in yog-kit as precompiled dialogs (`yog-kit/src/dialogs/*.yui` with
  committed `.g.rs`, regenerated by a yog-kit test):

  ```rust
  cx.message(Strings::saved()).show();                                          // OK
  cx.message(text).buttons(Buttons::YesNo).then(|vm, b| if b == Button::Yes { … });
  let ok = vm.cx().confirm(Strings::confirm_clear()).await;                     // YesNo → bool
  cx.prompt(Strings::rename()).initial(&name).validate(non_empty).then(|vm, s: String| vm.rename(s));
  ```

- **TestHost**: `h.dialog::<confirm_delete::View>()` returns a `TestLayer` with the same API as
  `TestHost` (click, text, vm). `h.answer(true)` closes the top dialog with a result.
  `h.messages()` lists the message boxes shown.

---

## 12. Messaging, server channels, persistence, server sync

### 12.1 Messenger (event aggregator)

```rust
#[derive(Clone)] pub struct PatternDrawn(pub String);          // any Clone + Send + 'static type is a message

yog_kit::publish(PatternDrawn(sig));                            // from anywhere: a Surface, another VM, a handler
cx.publish(ModsChanged);                                         // the same, from a command
```

- Delivered to every **registered** screen VM of the mod that has `#[on(T)]`. A closed screen's VM
  receives it too, because VMs live as long as their registration. Use `#[on(T, when_open)]` to
  skip closed screens.
- Delivery happens on the render thread. If the target host is free it runs immediately,
  otherwise it's queued. yog-kit registers one `on::<ClientTick>` per mod that drains queues of
  closed screens. This fixes a v1 gap: `ScreenHandle::update` queues only drain while a screen is
  open, and grow while it's closed.
- **Cross-mod messages** use D1: `#[derive(Codec, Message)] #[message(topic = "hexcasting:report")]`
  plus `yog_kit::publish_global(&msg)`. They travel as Codec bytes through the interop layer and
  are decoded on the receiving side's `#[on(T)]`. This needs a runtime broadcast hook, so it
  comes in a later stage (§16, M9).

### 12.2 Server channels into VMs: `#[recv(CHANNEL)]`

With F3, `#[recv(protocol::NEW_PATTERN_RESULT)] fn result(&mut self, res: PatternResult)` makes
the generated `register()` call `registry.on_client(NEW_PATTERN_RESULT, …)`, forwarding the
decoded payload to the VM as a tracked message. HexMod's `on_result()` / `open()` free functions,
their `static Mutex` session and the `on_client_packet` registrations disappear (§15.2). `ScreenHandle::update` remains for code outside
the VM, but is no longer the normal path.

### 12.3 Persisted VM state: `#[persist]`

`#[persist]` on a field (whose type is `Codec`) saves it client-side to
`<gameDir>/yog/ui-state/<mod>/<screen>.bin` on close and on game exit, and loads it before the
first `Opened`. It's for UI preferences: the Modlist filter, a last tab, column widths. An
undecodable blob is ignored with a warning (WorldData's policy from F4).

**Hot reload** (optional, Q11): `#[view_model(reload)]` on a VM whose fields are all `Codec`
encodes the whole VM before `hot_reload` and decodes it after. This fixes "hot reload loses VM
state" from yog-kit.md §15.

### 12.4 Server data sync for container screens: `Synced<T>`

For screens backed by server state, whether a yog-inventory container menu (real slots) or any
screen showing block-entity or player data:

```rust
// shared (both sides)
#[derive(Codec, Clone, PartialEq, Default)]
pub struct CrucibleState { pub heat: u32, pub max_heat: u32, pub progress: f32, pub mode: Mode }
#[derive(Codec)] pub enum CrucibleCmd { SetMode(Mode), Vent }

pub static CRUCIBLE: MenuSync<CrucibleState, CrucibleCmd> = MenuSync::new("mymod:crucible");

// server
registry.on_menu_open(CRUCIBLE, |pos: &DimPos, cx| crucible_state(pos));     // initial snapshot
registry.on_menu_cmd(CRUCIBLE, |cmd, cx| match cmd {                          // cx: player, menu, server
    CrucibleCmd::SetMode(m) => cx.with_state(|s| s.mode = m),
    CrucibleCmd::Vent => vent(cx.pos(), cx.server()),
});
CRUCIBLE.push(&pos, |s| s.heat = heat);   // from server logic; batched per server tick

// client VM
#[view_model] #[derive(Default)]
pub struct CrucibleVm { state: Synced<CrucibleState> }
#[view_model] impl CrucibleVm {
    #[command(can = self.state.heat > 0)]
    fn vent(&mut self) { self.state.send(CrucibleCmd::Vent); }
}
```

```xml
<Label text="{x:Bind state.heat, Format='{} °'}"/>
<Slider value="{x:Bind state.progress}" enabled="false"/>
<TabBar selected_tab="{x:Bind state.mode, BindBack=set_mode}"/>
<ContainerSlots/>                     <!-- where yog-inventory's real slots go; the layout positions them -->
```

- `MenuSync` is in yog-api (both sides). On the server, per open menu per player, it keeps the
  last sent `Codec` bytes. At the end of a server tick a changed state is sent as one S2C message:
  a whole snapshot in v1, with a field-level delta later (Q12). On the client, `Synced<T>`
  (`Deref<Target = T>`) is replaced as a whole and its field bit becomes dirty, so only dependent
  bindings re-run.
- Client→server goes only through `CrucibleCmd`, which is C2S. The server checks that the player
  has that menu open, so a VM can't write server state directly. There is no prediction: the UI
  shows server state.
- `<ContainerSlots/>` reports its laid-out rect so that yog-inventory's `SlotLayout`s are
  positioned relative to it (`InventoryDef::background_size` today). That's the bridge between
  `.yui` layout and real vanilla slots. Exact Java plumbing belongs to yog-inventory's phases.
- Without a container, `Synced<T>` also works with a `PlayerData<T>` (F4) mirrored to the client
  while the screen is open.

### 12.5 Opening screens: `open_on=` and `menu_entry=`

There are three ways a screen opens, and none of them needs code:

```xml
<Screen id="yog-modlist:mod_list" menu_entry="@res:Strings.menu_button" …/>        <!-- F8: a button in the vanilla menu -->
<Screen id="hexcasting:spellcasting" open_on="protocol::OPEN_GUI" …/>           <!-- the server sends S2C<OpenGui> -->
<Screen id="mymod:atlas" open_key="key.mymod.atlas" …/>                         <!-- a key binding (registered + translatable) -->
```

- `open_on="CHANNEL"` registers `on_client(CHANNEL, …)` in the generated `register()`. It creates
  or reuses the VM, delivers the payload to `#[on(Opened<P>)] fn init(&mut self, p: P)` when the
  VM has one, and opens the screen. The server side is one line where the logic is:
  `protocol::OPEN_GUI.send_to(player, &gui)`. This replaces HexMod's `on_client_packet(CH_OPEN, …)`
  + `open()` + `client::open_ui_flags(..)` (§15.2).
- Screen flags (`pause=`, `background="none"`, `modal=`) are root attributes, so code never
  calls `open_ui_flags`.
- `close_when="{x:Bind …}"` closes the screen when a binding becomes true. It's checked every
  client tick (the binding's mask includes the `TICK` bit when it calls `#[volatile]` state). For
  example, `close_when="{x:Bind !holding_staff()}"`.

### 12.6 Settings screens generated from a config struct

The most common mod UI is a config screen. With D1, a config is just a `Codec` struct, and its
screen is derived:

```rust
#[derive(Codec, Config, Default)]
#[config(file = "client", screen)]                    // <gameDir>/config/<mod>/client.bin + a generated screen
pub struct ClientConfig {
    /// Grid dot density.                              // doc comment → tooltip (through Tr keys, F6)
    #[setting(range = 0.5..=2.0, step = 0.1)] pub grid_scale: f32,
    #[setting] pub show_ravenmind: bool,
    #[setting] pub stack_side: Side,                   // enum → TabBar or a cycling button
    #[setting(color)] pub pattern_color: u32,
    #[setting(section = "sounds", range = 0..=100, format = "{}%")] pub ambiance_volume: u8,
}

// anywhere: read and write
let scale = ClientConfig::get().grid_scale;
```

- `#[derive(Config)]` (yog-macros) generates: load and save with Codec, a `get()`/`update(f)`
  accessor, a `ConfigChanged<ClientConfig>` event (D2), and a **generated settings screen**: a
  VM wrapping a working copy, a `.yui`-equivalent view (a label + control per field, grouped by
  `section`, with Reset and Done), validation from `range`, and a `menu_entry` in the mod's
  entry of the platform's mod list (Fabric ModMenu, Forge/NeoForge config button) where the
  loader exposes one.
- Control choice by type: `bool` → CheckBox, numbers with `range` → Slider, other numbers →
  TextBox with `Parse`, `String` → TextBox, field-less enums → TabBar (≤ 4 variants) or a cycling
  button, `#[setting(color)]` → a color picker, `Vec<T>` → a list editor with add and remove,
  nested structs → a section.
- To customise the layout, write `client_config.yui` with
  `x:ViewModel="yog_kit::ConfigVm<ClientConfig>"`. It binds the same generated VM, so only the
  markup is hand-written.
- `#[config(server)]` makes it a server config: stored in the world (F4 `WorldData`), synced to
  operators through `Synced<T>` (§12.4), and editable only by them. The generated commands send
  the changed fields C2S, and the server re-validates with the same `range` rules.

### 12.7 Container menus declared in markup

A container screen (a block with slots) currently needs an `InventoryDef` in Rust plus separate
UI code. In v2, the `.yui` declares both, and yog-cli generates the `InventoryDef`:

```xml
<Menu id="mymod:crucible" x:ViewModel="CrucibleVm" sync="CRUCIBLE" width="176" height="166">
    <Row padding="8" gap="6">
        <SlotGrid name="input" columns="3" rows="1" filter="mymod:meltable"/>
        <Column gap="2">
            <ProgressArrow value="{x:Bind state.progress}"/>
            <Label text="{x:Bind state.heat, Format='{} °'}"/>
            <Button text="@res:Strings.vent" command="{x:Bind vent()}"/>
        </Column>
        <Slot name="output" output="true"/>
    </Row>
    <PlayerInventory/>                                 <!-- the standard 3×9 + hotbar at the bottom -->
</Menu>
```

- `<Slot>` / `<SlotGrid>` / `<PlayerInventory>` become `SlotLayout`s positioned by the `.yui`
  layout, plus the shift-click transfer rules (input slots ← player inventory, output → player).
  The server-side `InventoryDef`, the menu type registration and the client screen are all
  generated from this one file.
- `sync="CRUCIBLE"` binds the `MenuSync` from §12.4, so `state.*` bindings and `vent()` work
  as shown there.
- The block entity's server logic stays hand-written (that's the mod's logic): what melts, how
  heat changes. It reads and writes slots through the generated typed accessors
  (`menu.input()`, `menu.output()`).

This depends on yog-inventory's Java phases (real MC menus).

### 12.8 Custom-drawn surfaces owned by the VM

Today a `<Surface source="hexcasting:spellcasting_grid">` is a global: the mod registers an
`Arc<dyn Surface>` once (`Once`), and since `paint`/`pointer` get `&self`, its state lives in a
`static Mutex<Session>` that the VM, the packet handlers and the surface all lock. v2 binds the
surface to the VM:

```rust
#[view_model]
#[derive(Default)]
pub struct SpellcastingVm { grid: Grid }                // Grid: plain struct, implements SurfaceView

impl SurfaceView for Grid {
    type Out = Effect;                                  // what the surface asks the VM to do
    fn paint(&self, gfx: &GfxContext, rect: Rect);
    fn pointer(&mut self, ev: Pointer, size: (f32, f32)) -> Option<Effect>;
    fn scroll(&mut self, dy: f32, size: (f32, f32)) -> Option<Effect> { None }
}
```

```xml
<Surface w="100%" h="100%" source="{x:Bind grid}" output="{x:Bind apply}"/>
```

- The host calls `paint(&vm.grid)` while painting and `pointer(&mut vm.grid)` during input
  dispatch, with no lock. The returned `Out` is passed to the `output=` command (`fn apply(&mut
  self, e: Effect)`), so the effects (sounds, sending the pattern) are ordinary VM logic.
- The write mask of a pointer event is the surface field's bit, so bindings over `grid` update.
- The id-based `Surface` registry stays for surfaces drawn by other mods or the designer, but a
  mod no longer needs it for its own screens.

---

## 13. Crate placement

| Crate | Gets | Loses |
|---|---|---|
| `yog-macros` (renamed `yog-interop`, F2) | `#[view_model]` (struct + impl), helper attributes `command`, `computed`, `on`, `recv`, `validate`, `persist`, `design`, `volatile`, `#[derive(Message)]`, `#[derive(Config)]` + `#[setting]` (§12.6) | — |
| `yog-kit` | `VmCore`, `Mask`, the masked host loop and shadow verification, `ObsVec`, `Loadable`, command state, the executor + `background` + `sleep`, gesture routing, validation store and rules, `ErrorText`, messenger, `#[persist]` storage, layers/dialogs/navigation, `MessageBox` dialogs, `Synced<T>` (client half), `events::*`, `TestHost` additions, `SurfaceView` (§12.8), `ConfigVm<T>` and the generated settings view (§12.6), `open_on`/`close_when` runtime; re-exports the macros | `ViewModel` hook methods (generated now), `navigate_to`, `Fallback` handling |
| `yog-ui` | `StateFlags::INVALID`; `keys::Gesture` parsing (shared with codegen tests); `ViewEvent::ValueChanged` part kinds for new controls (with the materials doc's controls) | — |
| `yog-api` | `MenuSync` + `on_menu_open`/`on_menu_cmd` (server half), `Rpc<Req, Resp>` (F3 extension), the `LanguageChanged` client event, `BindText for Text` (or in yog-kit) | — |
| `yog-cli` | split `yui/` modules (§5.10), masks, can-execute, gestures, new binding grammar, element refs, scopes, template `match`, compiled controls, schema check, diagnostic remapping, `CODEGEN = 2`, `open_on=`/`open_key=`/`close_when=`, `<Menu>`/`<Slot>`/`<SlotGrid>`/`<PlayerInventory>` → `InventoryDef` (§12.7), boilerplate lints (§3.0) | non-VM screen codegen, `on_*` attributes, `<Event>`, component rebuild path, `Fallback=` |
| runtime / ABI | language-change notification to UI handlers; later: cross-mod message broadcast, menu sync ids | — |
| Yog-IDLE | §14 | event-handler code-behind generation |

None of this touches the `YogApi` table except the later runtime items (`LanguageChanged`,
cross-mod broadcast, menu sync), each an ABI_MINOR append.

---

## 14. Designer support (Yog-IDLE)

1. **Design-time DataContext with real code.** IDLE already builds the mod for the host machine
   (`yog designer-lib`) to draw code-backed components. v2 extends that. A VM may provide sample
   data:

   ```rust
   #[view_model] impl ModListVm {
       #[design] fn sample() -> Self { Self { mods: vec![ModEntry::sample("alpha"), …], ..Default::default() } }
   }
   ```

   The designer-lib exports a headless host (the `TestHost` machinery), and the canvas draws
   **the real view with real bindings** over the sample VM. Preview mode then runs real commands
   on the sample VM, and Reset restores the sample.
   Without a build (or before the first one), the fallback is today's plan: `d:` attribute values,
   then `d:Sample="mod_list.sample.json"` (JSON keyed by binding expression), then grey `‹expr›`
   placeholders.
2. **Schema.** The macro also emits a static `__YK_SCHEMA` (fields with type names, computed,
   commands with parameter types and can-execute, validators, events handled), exported by the
   designer-lib for exact information. `yog ui schema` (syn) gives instant, approximate data while
   typing. Both feed:
   - a **Binding picker**: a tree of VM members (or item members inside templates, or element
     state under `#name`), Mode/Format/converter fields, and a live value preview from the sample
     VM;
   - a **Command picker** for `command=`: `#[command]`s with parameter editors, can-execute
     preview ("disabled with the sample data"), and *New command…*, which inserts
     `#[command] fn name(&mut self) {}` into the `#[view_model] impl` and opens it. This replaces
     today's double-click-to-event-handler in `codebehind.rs` with double-click-to-command;
   - a **Gesture editor** on the screen's InputBindings (press-to-record, conflict warnings);
   - XAML completion inside `{x:Bind |}`, `#` and `cmd.`.
3. **State preview**: toggles for interaction states, `invalid`, and each `style:Key` conditional
   style of the selected element. Validation preview shows sample errors (`d:errors.name="…"`).
4. **Template selectors**: the canvas shows one sample row per `match` arm, labelled with its
   pattern.
5. **Dialogs**: a `<Dialog>` root opens like a screen. The parent screen's *Show dialog* preview
   overlays it.
6. **Diagnostics**: the name check (§5.8) runs in IDLE on every edit (it's syn plus the markup
   parser, with no build), giving squiggles in the XAML pane. rustc errors arrive remapped after
   a build.

---

## 15. Migration

### 15.1 Yog-Modlist

The layout itself is being redone for the vanilla look (materials §12.1). Only the MVVM parts are
shown here.

**Before** (`mod_list.rs`, abbreviated):

```rust
#[derive(Default)]
pub struct ModListVm { mods: Vec<ModEntry>, filter: Filter, search: String, selected: Option<String> }

impl ViewModel for ModListVm {
    fn on_open(&mut self, _cx: &mut Cx<'_, Self>) { if self.mods.is_empty() { self.mods = load_mods(); } }
}
impl ModListVm {
    pub fn visible(&self) -> impl Iterator<Item = &ModEntry> + '_ { /* filters on every call */ }
    pub fn has(&self, f: Filter) -> bool { … }
    pub fn count_text(&self) -> String { … }
    pub fn empty_text(&self) -> String { … }
    pub fn selected(&self) -> Option<&ModEntry> { … }
    pub fn selected_id(&self) -> Option<&str> { … }
    pub fn list_scope(&self) -> (Filter, &str) { … }
    pub fn select(&mut self, id: String) { self.selected = Some(id); }
    pub fn set_filter(&mut self, filter: Filter) { self.filter = filter; }
}
// ModSource::label/badge/color: hand-written matches (F6 replaces them with #[derive(Tr)])
```

```xml
<ToggleButton x:Name="filter_yog" style="@Filter" text="@res:Strings.filter.yog"
              checked="{x:Bind filter == Filter::Yog}" enabled="{x:Bind has(Filter::Yog)}"
              command="{x:Bind set_filter(Filter::Yog)}"/>
<ListView … items="{x:Bind visible()}" key="{x:Bind id}" selected="{x:Bind selected_id(), BindBack=select}"
          scroll_key="{x:Bind list_scope()}" item_height="24">
```

**After:**

```rust
#[view_model]
#[derive(Default)]
pub struct ModListVm {
    mods: Vec<ModEntry>,
    #[persist] filter: Filter,          // Filter: Codec
    search: String,
    selected: Option<String>,
}

#[view_model]
impl ModListVm {
    #[on(Opened)]
    fn load(&mut self) { if self.mods.is_empty() { self.mods = load_mods(); } }

    #[computed]
    fn visible_ix(&self) -> Vec<u32> {
        let q = self.search.trim().to_lowercase();
        (0..self.mods.len() as u32).filter(|&i| { let m = &self.mods[i as usize]; self.filter.allows(m) && m.matches(&q) }).collect()
    }
    fn visible(&self) -> impl Iterator<Item = &ModEntry> { self.visible_ix().iter().map(|&i| &self.mods[i as usize]) }
    fn has(&self, f: Filter) -> bool { … }                    // unchanged
    fn count_text(&self) -> String { … }                      // unchanged
    fn selected(&self) -> Option<&ModEntry> { … }             // unchanged

    #[command(can = self.has(filter))]                        // `enabled=` disappears from markup
    fn set_filter(&mut self, filter: Filter) { self.filter = filter; }
    fn select(&mut self, id: String) { self.selected = Some(id); }
    fn select_next(&mut self) { … }                            // for Down in the search box
    fn select_prev(&mut self) { … }
}
```

```xml
<Screen id="yog-modlist:mod_list" … x:ViewModel="ModListVm" menu_entry="@res:Strings.menu_button">
    <Screen.InputBindings>
        <KeyBinding gesture="Ctrl+F" command="{x:Bind cx.focus(ids::SEARCH)}"/>
    </Screen.InputBindings>
    …
    <TextBox x:Name="search" text="{x:Bind search, Mode=TwoWay, Delay=150ms}" placeholder="@res:Strings.search.placeholder">
        <TextBox.InputBindings>
            <KeyBinding gesture="Down" command="{x:Bind select_next()}"/>
            <KeyBinding gesture="Up" command="{x:Bind select_prev()}"/>
        </TextBox.InputBindings>
    </TextBox>
    <ToggleButton x:Name="filter_yog" text="@res:Strings.filter.yog"
                  checked="{x:Bind filter == Filter::Yog}" command="{x:Bind set_filter(Filter::Yog)}"/>
    <ListView x:Name="mod_list" items="{x:Bind visible()}" key="{x:Bind id}"
              selected="{x:Bind selected()?.id, BindBack=select}"
              scroll_key="{x:Bind (filter, search)}" item_height="36">   <!-- tuple literal: see Q6 -->
        <DataTemplate x:DataType="ModEntry"> … </DataTemplate>
        <ListView.Empty><Label text="{x:Bind empty_text()}"/></ListView.Empty>
    </ListView>
    <ScrollView x:Name="details" scroll_key="{x:Bind selected()?.id}">
        <ContentPresenter content="{x:Bind selected()}"> … </ContentPresenter>
    </ScrollView>
</Screen>
```

The `ListView.Empty` label keeps calling `empty_text()`, because bindings deliberately have no
ternary. `selected_id()` and `list_scope()` go away (`?.` and the tuple key replace them).
`impl ViewModel` goes away. The test becomes:

```rust
let mut h = TestHost::opened(ModListVm { mods, ..Default::default() });   // F12
h.click_id(ids::FILTER_PLATFORM);
assert!(h.state(ids::FILTER_YOG).contains(StateFlags::DISABLED) == !h.vm().has(Filter::Yog));
h.press("Ctrl+F"); h.type_text("beta"); h.press("Down");
assert_eq!(h.vm().selected.as_deref(), Some("beta"));
// every step above also ran the tracking verification (§3.2)
```

### 15.2 HexMod-Yog: the staff's casting grid

HexMod's UI is now a port of the original `GuiSpellcasting` (HexMod commit `6712150`): one
full-screen `<Surface>` in `src/ui/spellcasting.yui`, drawn by `GridSurface` in
`src/ui/spellcasting.rs`. The old `casting_screen` with lists, components and a Clear button is
gone, so the materials doc's §12.2 restyle of it no longer applies. This screen is the
"custom-drawn surface + server packets" case, and it shows the glue that §12.5 and §12.8 remove.

**Before** (`spellcasting.rs`, the parts that aren't grid logic):

```rust
mod view { use super::SpellcastingVm; yog_api::ui_include!("ui/spellcasting"); }
pub use view::{handle, register, ID};
use view::close;

struct Session { grid: Option<Grid>, ambiance: SoundHandle, closing: bool }
static SESSION: Mutex<Session> = Mutex::new(Session { grid: None, ambiance: SoundHandle(0), closing: false });
fn session() -> MutexGuard<'static, Session> { SESSION.lock().unwrap_or_else(|e| e.into_inner()) }

struct GridSurface;
impl Surface for GridSurface {
    fn paint(&self, gfx: &GfxContext, rect: Rect) {
        let mut s = session();
        let Some(grid) = s.grid.as_mut() else { return };
        // `tick` hidden in paint: close when the staff leaves the hand
        let held = client::held_item(grid.hand == Hand::Off);
        if !held.is_some_and(|id| id.starts_with("hexcasting:staff/")) && !s.closing { s.closing = true; close(); }
        … drawing …
    }
    fn pointer(&self, ev: Pointer, size: (f32, f32)) -> bool { let effect = { let mut s = session(); … }; Self::apply(effect); true }
    fn scroll(&self, dy: f32, _: (f32, f32)) -> bool { … session() … }
}

#[derive(Default)] pub struct SpellcastingVm;
impl ViewModel for SpellcastingVm {
    fn on_open(&mut self, _cx: &mut Cx<'_, Self>) { let mut s = session(); client::stop_sound(s.ambiance); s.ambiance = client::play_sound(SOUND_AMBIANCE, 1.0, 1.0, true); }
    fn on_close(&mut self, _cx: &mut Cx<'_, Self>) { client::stop_sound(std::mem::take(&mut session().ambiance)); }
    fn on_back(&mut self, _cx: &mut Cx<'_, Self>) -> bool { session().grid.as_mut().is_some_and(Grid::cancel_stroke) }
}

pub fn open(gui: OpenGui) {
    static SURFACE: Once = Once::new();
    SURFACE.call_once(|| yog_api::ui::surface::register(SURFACE_ID, Arc::new(GridSurface)));
    { let mut s = session(); s.grid = Some(Grid::new(gui)); s.closing = false; }
    client::open_ui_flags(ID, ui_flags::MODAL | ui_flags::NO_BACKGROUND);
}

pub fn on_result(res: PatternResult) {
    let keep = { let mut s = session(); let Some(grid) = s.grid.as_mut() else { return }; grid.recv_server_update(res) };
    if !keep { close(); }
}
// + hand-written packet decoding and registration in events/mod.rs / lib.rs,
// + `OpenGui::encode/decode`, `NewPattern::encode`, `Writer` for CH_SCROLL (D1 Codec replaces them)
```

**After:**

```rust
#[view_model]
#[derive(Default)]
pub struct SpellcastingVm {
    grid: Grid,                    // implements SurfaceView (§12.8); draw_* / recv_server_update unchanged
    ambiance: SoundHandle,
}

#[view_model]
impl SpellcastingVm {
    #[on(Opened<OpenGui>)]                                   // open_on="protocol::OPEN_GUI"
    fn start(&mut self, gui: OpenGui) {
        self.grid = Grid::new(gui);
        client::stop_sound(self.ambiance);
        self.ambiance = client::play_sound(SOUND_AMBIANCE, 1.0, 1.0, true);
    }
    #[on(Closed)] fn stop(&mut self) { client::stop_sound(std::mem::take(&mut self.ambiance)); }
    #[on(Back)]   fn back(&mut self) -> Flow { Flow::handled_if(self.grid.cancel_stroke()) }

    #[recv(protocol::NEW_PATTERN_RESULT)]                    // S2C<PatternResult>, Codec
    fn result(&mut self, res: PatternResult, cx: &mut Cx<Self>) {
        if !self.grid.recv_server_update(res) { cx.close(); }
    }

    fn apply(&mut self, effect: Effect) {                    // the Surface's output=
        match effect {
            Effect::StartSound => { client::play_sound(SOUND_START, 0.25, 1.0, false); }
            Effect::SegmentSound => { client::play_sound(SOUND_ADD_SEGMENT, 0.25, 1.0 + (random() - 0.5) * 0.1, false); }
            Effect::Send(msg) => protocol::NEW_PATTERN.send(&msg),                     // C2S<NewPattern>
            Effect::Scroll(main, off) => protocol::SCROLL.send(&Scroll { main, off, sprint: client::control_down() }),
        }
    }

    #[volatile] fn holding_staff(&self) -> bool {
        client::held_item(self.grid.hand == Hand::Off).is_some_and(|id| id.starts_with("hexcasting:staff/"))
    }
}
```

```xml
<Screen id="hexcasting:spellcasting" anchor="fill" pause="false" background="none" modal="true"
        open_on="protocol::OPEN_GUI" close_when="{x:Bind !holding_staff()}" x:ViewModel="SpellcastingVm">
    <Surface x:Name="grid" w="100%" h="100%" source="{x:Bind grid}" output="{x:Bind apply}"/>
</Screen>
```

Removed: `Session`, `SESSION`, `session()`, `GridSurface` and its three lock-and-forward methods,
`Once` surface registration, `open()`, `on_result()`, `impl ViewModel`, the `mod view` re-export
dance, the client packet registrations in `events/mod.rs`, and the hand-written wire
encode/decode for these packets (D1). The close-when-staff-leaves check moves out of `paint`
into `close_when=`. The server side keeps `vm::handle_cast` and `vm::reset` (logic), and its
packet registrations become `registry.on(protocol::NEW_PATTERN, …)` (F3).

The grid unit tests (`grid()`, `draw()`, …) don't change: `Grid` was already plain data. A
screen-level test becomes possible:

```rust
let mut h = TestHost::opened_with::<View>(OpenGui { hand: Hand::Main, ..Default::default() });
h.pointer_drag(ids::GRID, &[(100.0, 100.0), (120.0, 100.0), (130.0, 117.0)]);
assert_eq!(h.sent(protocol::NEW_PATTERN).len(), 1);
h.receive(protocol::NEW_PATTERN_RESULT, PatternResult { stack_clear: true, ..Default::default() });
assert!(h.closed());
```

### 15.3 Line counts

Measured on the current sources (`wc -l`, including doc comments, excluding tests). "After" is an
estimate from the sketches above with the current formatting.

| File | Before | After | What goes |
|---|---:|---:|---|
| Yog-Modlist `lib.rs` | 33 | ~6 | `impl Mod`, `register`, `register_menu_entry`, `ui_components!` (F8, `menu_entry=`) |
| Yog-Modlist `mod_list.rs` | 177 | ~115 | `impl ViewModel`, `selected_id`, `list_scope`, `ModSource::label/badge/color` (F6 `#[derive(Tr)]`); `visible`/`has` share the computed filter |
| Yog-Modlist `stat_bar.rs` (+ `pill.yui`, `stat_bar.yui`: 24) | 25 | 0 | deleted by the materials doc |
| Yog-Modlist `mod_list.yui` | 135 | ~125 | `enabled=` on filters (can-execute), helper calls replaced by `?.` and tuples |
| **Yog-Modlist Rust total** | **235** | **~120** | **about −50%**, and what's left is the entry model, filtering and loading |
| HexMod `ui/spellcasting.rs` (non-grid part: VM, session, surface, open/result) | ~150 | ~45 | §15.2 |
| HexMod `ui/spellcasting.rs` (grid logic, drawing) | ~530 | ~530 | logic stays |
| HexMod `events/mod.rs` client packet glue | 10 | 0 | `open_on=`, `#[recv]` |
| HexMod `protocol.rs` `write`/`read`/`encode`/`decode` for the UI packets | ~110 | ~0 | D1 `#[derive(Codec)]` (unless these packets must stay byte-compatible with something else, which they don't today: both ends are this mod) |

A new screen of the "form" or "list" kind, written against v2, is the VM struct and its
non-trivial commands: the `RenameVm` example in §3.0 is 12 lines of Rust. A settings screen is
0 lines beyond the config struct (§12.6).

---

## 16. Staged plan

Each stage ships on its own (crates bumped as breaking, published, the mods updated in the same
change, and `dev.sh` run only with the user's permission).

| Stage | Content | Depends on |
|---|---|---|
| **M0** Codegen cleanup | Split `yui_vm.rs` into `yui/` modules; delete the non-VM screen path (`Vm = ()`), the `on_*` attributes and `Fallback=`; per-row captured command arguments (O(1) item commands); cargo diagnostic remapping to `.yui`; `CODEGEN = 2`; IDLE: drop event-handler generation | — |
| **M1** `#[view_model]` core | yog-macros: struct and impl attribute, masks, R/W analysis, `ViewModel` generation, `#[on(Opened/Closed/Back/Tick/Customize)]`, `#[volatile]`; yog-kit: `VmCore`, masked host loop, shadow verification, `TestHost` verification + `opened`; codegen: masked `update`, `dispatch -> Mask`; migrate Modlist and HexMod (attributes only) | M0; boilerplate Stage 1 (the `yog-macros` rename) |
| **M2** Commands | `#[command(can)]` → `enabled`, `DISABLED`; gestures (`InputBindings`, `gesture=`, element scope, routing); `activate=`; `TestHost::press/exec/can_execute` | M1 |
| **M3** Bindings | grammar (`&&`, `\|\|`, comparisons, `??`, `[i]`, tuples); `Format=`; converter pipes and `Converter` (TwoWay `Parse`); `OneTime`/`OneWayToSource`; `Delay=`/`Trigger=`; `#name` refs; `data_context`; nested templates + `parent`; required `x:DataType`; `yog ui schema` + name checks | M1 (masks); parallel with M2 |
| **M4** Collections & templates | `ObsVec` + `ItemsHost::apply_changes`; template `match` selectors; multi-select | M1 |
| **M5** Computed & async | `#[computed]` memo; `Loadable` + `ContentPresenter.Loading/Failed`; executor, `background`, `sleep`, async commands, `cmd.*` state, `CommandFailed`, `cx.spawn` | M1, M2 |
| **M6** Validation & style hooks | `#[validate]`, conversion errors, `errors.*`, `ErrorText`, `INVALID`, `style:Key=` conditional styles | M3 (converters); coordinate `invalid` with materials S2/S5 |
| **M7** Compiled controls | components with typed props, `Command` properties, in-place patching, optional per-instance VM; designer-only `Props` path | M1, M2 |
| **M8** Navigation & dialogs (P10) | typed navigate, `navigate_for`/`Returned<R>`, layers, `<Dialog>`, callback form; `MessageBox` set; `TestLayer`; `await` form once M5 lands | M2; async form needs M5 |
| **M9** Messenger, channels, persistence | `publish` / `#[on(T)]`, closed-screen draining; `#[recv(CH)]` (needs F3); `#[persist]` (needs F2 Codec); cross-mod messages (needs D1 interop on Codec + a runtime broadcast hook); optional hot-reload VM carry | M1; boilerplate Stage 1 |
| **M9b** Declarative glue | `open_on=` + `Opened<P>`, `open_key=`, `close_when=`, root screen flags (§12.5); `SurfaceView` + `source="{x:Bind field}"` + `output=` (§12.8); `#[derive(Config)]` client configs + generated settings screen (§12.6); boilerplate lints (§3.0); migrate HexMod's spellcasting screen | M1, M9 (`#[recv]`); `menu_entry=` from F8 |
| **M10** Server sync | `MenuSync`, `Synced<T>`, `on_menu_open/cmd`, `<ContainerSlots/>`, `<Menu>` with `<Slot>`/`<SlotGrid>`/`<PlayerInventory>` generating `InventoryDef` (§12.7), `#[config(server)]` (§12.6), `Rpc<Req, Resp>` | M9; F3; F4 (`PlayerData` variant); yog-inventory Java phases |
| **M11** IDLE | `#[design]` sample VMs via designer-lib, `__YK_SCHEMA`, binding/command/gesture pickers, state + conditional-style + validation preview, template-arm preview, dialog preview, live name diagnostics | M1–M3 for the base; each later stage adds its panel |

Critical path: M0 → M1 → {M2, M3, M4} → M5 → M8. For the boilerplate goal, M9 → M9b gives the largest cut in existing mod code (HexMod) and should follow M2 directly. Materials S5 (new controls) is needed for the
two-way plumbing of CheckBox/Slider/TabBar in M3 but nothing else. MVVM can land before or after
it.

---

## 17. Risks

- **Tracker conservatism vs. surprise.** Over-approximation is harmless (extra evaluations).
  Under-approximation is only possible through interior mutability or globals, and the shadow
  pass reports it in debug builds and tests. The real risk is authors ignoring the warning; the
  message must name the binding and the method.
- **Proc-macro compile time.** One attribute per VM and impl block, with token scanning only (no
  type resolution). Small next to `serde`.
- **Rewritten signatures** (`#[computed]` returns `&T`) may confuse rust-analyzer users briefly.
  The schema and IDE show the real signature.
- **`??`, `&&` and friends** make markup more code-like. Mitigation: no ternaries, no
  arithmetic, no closures, ever. Methods remain the answer for logic.
- **Layers inside one MC screen** must handle the vanilla pause/Esc logic: Esc closes the top
  dialog and must not reach the host's own close. Covered by `on_event` ordering, but needs a
  manual test on all loaders.

---

## 18. Open questions for the user

1. **Q1 Tracking model — decided: static masks with plain fields.** The user asked for whichever is more
   convenient and works better and faster. Masks win on both: fields stay plain (`self.name = x`, no
   `.get()/.set()`, no wrapper types in literals or tests), tracking costs one `u64` OR per write and nothing per
   read, and there is no per-field allocation or subscriber list. The dynamic cases signals would catch
   (writes through opaque `&mut self` helpers outside the impl) fall back to "all fields dirty", which the
   `Slot` diff keeps correct; the debug shadow re-evaluation flags them. Was: static masks (recommended), or
   runtime signals (`Prop<T>` fields with `.get()/.set()`)?
2. **Q2 Hidden field — decided: no hidden field.** The core lives in the host and generated code
   reaches it through a scoped current-core pointer (§3.1), so struct literals stay plain and
   computed memo and `errors()`/`is_valid()` still work from inside the VM. Was: `__yk: VmCore` in
   the struct (literals need `..Default::default()`), or the core in the host without those.
3. **Q3 Gestures**: in markup only (recommended), or also `#[command(key = "Ctrl+S")]` in Rust?
4. **Q4 Async — decided: yes, built-in mini executor.** Was: is a built-in mini executor in yog-kit wanted (recommended: it enables
   `await dialog`, `Rpc` and `background`), or callbacks only?
5. **Q5 `Fallback=` → `??` — decided: yes, delete `Fallback=`.** Was: OK to delete `Fallback=` outright (no-backward-compat)?
6. **Q6 Binding grammar — decided: yes.** Was: add `&&`, `||`, `<`/`>`, `??`, `[i]` and tuple literals (recommended),
   given v1 deliberately refused them?
7. **Q7 Screens without `x:ViewModel` — decided: compile as `Vm = ()`.** Was: compile them as `Vm = ()` (recommended), or require a VM?
8. **Q8 `CollectionView`** (incremental filter/sort over `ObsVec`): needed now, or after real
   lists get large?
9. **Q9 Component VMs** (`x:ViewModel` on `<Component>`): include in M7, or keep components
   stateless?
10. **Q10 `invalid` state — decided: yes (bit 6 + `.ymat` `invalid`).** Was: agree to add bit 6 (`INVALID`) to `StateFlags` and an `invalid`
    state to `.ymat`, coordinated with the materials design?
11. **Q11 Hot reload**: carry VM state across hot reload when the VM is `Codec`
    (`#[view_model(reload)]`)?
12. **Q12 Server sync granularity — decided: whole snapshots first.** The server encodes the synced state with
    Codec once per tick and sends it only when the bytes changed; screen state is small (hundreds of bytes), so
    this costs less than tracking per-field deltas and has no ordering or desync bugs. Field-level deltas can be
    added behind the same `Synced<T>` API later if a real screen needs them. Was: whole-snapshot diffs per tick (recommended first), or
    field-level deltas from the start?
13. **Q13 Messages to closed screens**: deliver by default (recommended, so VMs stay current) or
    only to open screens?
14. **Q14 Settings screens — decided: yes, generate and hook into the platform config button.** Was: generate a whole config screen from `#[derive(Config)]` (recommended),
    and hook it into the platform's mod-list config button (Fabric ModMenu, Forge/NeoForge) where
    possible?
15. **Q15 Container menus in `.yui` — decided: yes, slot layout in markup generates `InventoryDef`.** (`<Menu>` + `<Slot>`s generating `InventoryDef`). Was: is markup the
    right place for slot layout, or should `InventoryDef` stay in Rust with `.yui` only drawing?
16. **Q16 VM-owned surfaces — decided: yes, for custom drawing (3D viewports and the like); 2D UI uses the declarative elements (no-manual-rendering.md R2).** (`source="{x:Bind grid}"`). Was: OK to make this the normal way a mod draws
    its own screen, keeping the id-based `Surface` registry only for cross-mod and designer use?
17. **Q17 Boilerplate lints — decided: warnings.** ("bind the field directly", "`impl ViewModel` is generated"): warnings
    in `yog check`, or errors?
