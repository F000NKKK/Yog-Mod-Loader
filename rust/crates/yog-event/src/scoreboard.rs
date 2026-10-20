//! The scoreboard: the host's actions.

use yog_ids as ids;

actions! {
    /// A score.
    Get { objective: String, player: String } -> Option<i32> = ids::service_scoreboard::GET, "scoreboardGet";
    /// Sets a score.
    Set { objective: String, player: String, score: i32 } -> bool = ids::service_scoreboard::SET, "scoreboardSet";
    /// Adds to a score; the new score.
    Add { objective: String, player: String, delta: i32 } -> Option<i32> = ids::service_scoreboard::ADD, "scoreboardAdd";
}
