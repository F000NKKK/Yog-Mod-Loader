# yog-inventory: настоящие Container/Menu экраны для Yog-модов

## Контекст

Workbench/ALU у Yog-VLSI сейчас — не настоящий Minecraft-инвентарь, а картинка
поверх экрана (`yog-ui`, флексбокс-виджеты, как у yog-book): ресурсы кладутся
только через "подержи предмет и кликни правой кнопкой по блоку", без слота,
drag-and-drop и визуального фидбека. Пользователь хочет вместо этого настоящий
инвентарный экран как у печки/Ender IO: свои слоты сверху + инвентарь игрока
снизу нативными текстурами, куда предмет реально перетаскивается мышкой.

Исследование показало: в загрузчике **вообще нет** концепции Minecraft
`BlockEntity` для собственных блоков модов (`get_block_nbt`/`set_block_nbt` —
мёртвый код, работает только на **ванильных** блок-энтити типа сундука).
`BlockDef` (`yog-registry/src/lib.rs:450`) — чисто визуальный, без хуков под
инвентарь. Ресурсы Yog-VLSI (`RESOURCES`, `ALU_STATE`) хранятся в обычных
`HashMap<(i32,i32,i32), _>`, сохраняются через `Storage::open(...)` и **никогда
не чистятся** при поломке блока (нет `on_block_break`-подписки) — то есть
"ресурсы не пропадают при поломке" технически уже выполняется, но случайно
(данные просто остаются висеть по координате, а не переезжают вместе с
предметом при поломке/установке, как у шалкер-бокса).

Значит `yog-inventory` — это НОВАЯ подсистема загрузчика с нуля:
Minecraft `BlockEntity` + `AbstractContainerMenu`/`ScreenHandler` +
`Screen`, добавленные на все 6 платформ (Fabric/Forge/NeoForge × 1.20.1/1.21.1).
Это большая работа — план разбит на фазы, чтобы двигаться проверяемыми шагами,
а не одним гигантским патчем.

## Требования (по всем сообщениям пользователя)

- Новый отдельный крейт `yog-inventory` в `rust/crates/`.
- Настоящие Minecraft-подобные (или полностью кастомные, как у Ender IO)
  инвентари — реальные слоты с drag-and-drop, а не оверлей.
- Экран включает часть с инвентарём **игрока** (основной инвентарь + хотбар,
  БЕЗ брони/офф-хэнда) — как надстройку над кастомными слотами мода.
- Дефолтные ванильные текстуры остаются (нативный вид), но мод может задать
  и свою фоновую текстуру.
- Маппинг сетки слотов (позиции) переопределяем через JSON-конфиг.
- Ресурсы/содержимое переживают поломку блока (по образцу шалкер-бокса —
  данные едут вместе с выпавшим предметом и возвращаются при установке).
- Крейт должен быть создан прямо сейчас как рабочая заглушка (даже до полной
  реализации), со всеми зависимостями подключёнными в workspace.
- В крейте — файл `DESIGN.md` с этим планом.

## Архитектура (по образцу существующих доменных крейтов)

Мод-часть (по аналогии с `BlockDef`/`ItemDef` в `yog-registry`, `UiRoot`/`widget`
в `yog-ui`, которые уже реэкспортируются через `yog-api`):

```rust
// yog-inventory
pub struct SlotLayout { pub x: f32, pub y: f32 }        // пиксели, ваниль. GUI-координаты
pub struct InventoryDef {
    pub id: String,                    // напр. "yog-vlsi:workbench_inv"
    pub slot_count: usize,
    pub layout: Vec<SlotLayout>,       // либо дефолтная сетка, либо из JSON
    pub include_player_inventory: bool,// основной инв. + хотбар, без брони
    pub player_inv_offset: (f32, f32),
    pub background_texture: Option<String>, // None = дефолтная ванильная
    pub title: String,
}
impl InventoryDef {
    pub fn new(id, slot_count) -> Self { .. }
    pub fn layout_from_json(mut self, path: &str) -> Self { .. } // remap сетки
    pub fn background_texture(mut self, path: &str) -> Self { .. }
    pub fn include_player_inventory(mut self, v: bool) -> Self { .. }
}
```

`Registry::register_inventory(InventoryDef)` + привязка к блоку через новое
поле `BlockDef.inventory_id: Option<String>` (аналог `connects`/`connect_groups`)
— именно так уже расширялся `BlockDef` для `connects`, паттерн знакомый.

Загрузчик (Java, 6 платформ, генерик-классы аналогично `YogConnectingBlock`/
`YogConnectingLogic`, где общая безъявная логика лежит в `java-common`, а
Minecraft-специфичный мост — по платформам):

- `YogInventoryBlockEntity` — держит массив стеков (per-platform: `ItemStackHandler`
  для Forge/NeoForge, `SimpleInventory`-based для Fabric), NBT save/load.
- Один `YogInventoryBlockEntityType`, валидный для ВСЕХ блоков с `inventory_id`
  (как один `YogConnectingBlock` переиспользуется для многих id — генерик,
  не по одному классу на блок).
- `YogInventoryMenu extends AbstractContainerMenu` / `ScreenHandler` — строит
  слоты из `InventoryDef` (запрошенного один раз через новый
  `NativeBridge.nativeInventoryDefs()`, по аналогии с `nativeBlockDefs()`) +
  стандартные слоты инвентаря игрока (`Slot`/`PlayerInventory`, ванильные
  текстуры/поведение бесплатно от MC).
- `YogInventoryScreen extends AbstractContainerScreen`/`HandledScreen` — рисует
  фон (дефолтный `generic_54`-стиль ИЛИ кастомная текстура) + слоты по layout.
- Открытие меню — `player.openMenu(menuProvider)` при right-click блока с
  `inventory_id` (никакого пакет-моста не нужно — это встроенный ванильный
  механизм синхронизации слотов).
- Поломка блока: сохранить NBT инвентаря в выпавший `ItemStack`
  (1.20.1: `BlockItem.setBlockEntityData`; 1.21.1: `DataComponents.BLOCK_ENTITY_DATA`
  — разные API, проверить по декомпилированным исходникам как раньше в сессии)
  и восстановить при установке — ровно как у шалкер-бокса.

## Фазы

1. **Заглушка крейта (сделать сразу)** — `rust/crates/yog-inventory/{Cargo.toml,src/lib.rs,DESIGN.md}`
   с пустыми/минимальными типами (`InventoryDef`, `SlotLayout` — компилируется,
   ничего не делает), прописать в `rust/Cargo.toml` (`members` + `[patch.crates-io]`),
   подключить зависимостью в `yog-api/Cargo.toml` + реэкспорт в `yog-api/src/lib.rs`
   (как сделано для `yog-ui`). Проверить `cargo check --workspace`.
2. **Модель данных** — `InventoryDef`/`SlotLayout` полностью, JSON-remap сетки,
   ABI-структуры в `yog-abi`, JNI (`yog-runtime`) для `nativeInventoryDefs()` +
   чтения/записи слота из Rust (`srv.get_inventory_slot`/`set_inventory_slot`
   через `Server`-трейт, аналог `get_held_item_nbt`).
3. **BlockEntity на одной платформе (Fabric 1.21.1)** — генерик block-entity +
   type, привязка к блоку с `inventory_id`, компиляция + ручной тест открытия
   пустого меню.
4. **Menu/Screen на той же платформе** — реальные слоты + рендер (дефолт и
   кастомная текстура) + инвентарь игрока снизу.
5. **Реплицировать на остальные 5 платформ** (Fabric 1.20.1, Forge×2, NeoForge×2) —
   основная масса работы, с проверкой API различий по декомпилированным jar
   (как раньше в сессии для networking).
6. **Break-preserves-contents** (шалкер-бокс паттерн), отдельно проверить
   1.20.1 (NBT) vs 1.21.1 (Data Components) API.
7. **Перевести Yog-VLSI Workbench** на `yog-inventory` вместо текущего
   "подержи предмет + правый клик".

## Проверка

- После фазы 1: `cargo check --workspace` в `rust/` — крейт собирается, ничего
  не ломает.
- После фаз 3–4 (одна платформа): реальный in-game тест — открыть меню правым
  кликом, положить/вынуть предмет мышкой, убедиться что слот инвентаря игрока
  выглядит нативно.
- После фазы 6: сломать блок с предметами в кастомных слотах → поднять
  выпавший предмет → поставить блок → предметы на месте.
- Компиляция всех 6 платформ (`./gradlew compileJava` с нужным `JAVA_HOME`,
  как делалось весь сеанс) после каждой реплицированной платформы.

Это большая многофазная работа — после фазы 1 (заглушка) стоит согласовать,
двигаться ли сразу к фазе 2/3, или начать с одной платформы end-to-end как
"вертикальный срез" перед тиражированием на остальные 5.
