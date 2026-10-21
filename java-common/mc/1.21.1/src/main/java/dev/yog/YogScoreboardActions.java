package dev.yog;

/** The host's scoreboard actions: the methods {@code yog_event::scoreboard} names. */
public final class YogScoreboardActions {
    private YogScoreboardActions() {}

    @YogServiceHost
    public static int scoreboardGet(String objective, String player) { return 0; }
    @YogServiceHost
    public static boolean scoreboardSet(String objective, String player, int score) { return false; }
    @YogServiceHost
    public static int scoreboardAdd(String objective, String player, int delta) { return 0; }
}
