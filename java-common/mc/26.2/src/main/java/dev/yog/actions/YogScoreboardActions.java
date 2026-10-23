package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogServiceHost;

import net.minecraft.server.MinecraftServer;

/** The host's scoreboard actions: the methods {@code yog_event::scoreboard} names. */
public final class YogScoreboardActions {
    private YogScoreboardActions() {}

    /** Score of {@code player} on {@code objective}, or {@code Integer.MIN_VALUE} if unknown. */
    @YogServiceHost
    public static int scoreboardGet(String objective, String player) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.Scoreboard sb = s.getScoreboard();
        net.minecraft.world.scores.Objective obj = sb.getObjective(objective);
        if (obj == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.ReadOnlyScoreInfo sc =
                sb.getPlayerScoreInfo(net.minecraft.world.scores.ScoreHolder.forNameOnly(player), obj);
        return sc == null ? 0 : sc.value();
    }

    @YogServiceHost
    public static boolean scoreboardSet(String objective, String player, int score) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return false;
        net.minecraft.world.scores.Scoreboard sb = s.getScoreboard();
        net.minecraft.world.scores.Objective obj = sb.getObjective(objective);
        if (obj == null) return false;
        sb.getOrCreatePlayerScore(net.minecraft.world.scores.ScoreHolder.forNameOnly(player), obj).set(score);
        return true;
    }

    /** Returns new score, or {@code Integer.MIN_VALUE} if objective unknown. */
    @YogServiceHost
    public static int scoreboardAdd(String objective, String player, int delta) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.Scoreboard sb = s.getScoreboard();
        net.minecraft.world.scores.Objective obj = sb.getObjective(objective);
        if (obj == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.ScoreAccess score =
                sb.getOrCreatePlayerScore(net.minecraft.world.scores.ScoreHolder.forNameOnly(player), obj);
        score.add(delta);
        return score.get();
    }
}
