//! Boss bars: the host's actions.

use yog_ids as ids;

actions! {
    /// Creates boss bar `id`.
    Create { id: String, title: String, color: String, style: String } -> bool = ids::service_bossbar::CREATE, "YogBossbarActions.bossbarCreate";
    /// Removes boss bar `id`.
    Remove { id: String } -> bool = ids::service_bossbar::REMOVE, "YogBossbarActions.bossbarRemove";
    /// Sets a boss bar's title.
    SetTitle { id: String, title: String } -> bool = ids::service_bossbar::SET_TITLE, "YogBossbarActions.bossbarSetTitle";
    /// Sets a boss bar's progress (0–1).
    SetProgress { id: String, progress: f32 } -> bool = ids::service_bossbar::SET_PROGRESS, "YogBossbarActions.bossbarSetProgress";
    /// Sets a boss bar's color.
    SetColor { id: String, color: String } -> bool = ids::service_bossbar::SET_COLOR, "YogBossbarActions.bossbarSetColor";
    /// Shows a boss bar to `player`.
    AddPlayer { id: String, player: String } -> bool = ids::service_bossbar::ADD_PLAYER, "YogBossbarActions.bossbarAddPlayer";
    /// Stops showing a boss bar to `player`.
    RemovePlayer { id: String, player: String } -> bool = ids::service_bossbar::REMOVE_PLAYER, "YogBossbarActions.bossbarRemovePlayer";
    /// Shows or hides a boss bar.
    SetVisible { id: String, visible: bool } -> bool = ids::service_bossbar::SET_VISIBLE, "YogBossbarActions.bossbarSetVisible";
}
