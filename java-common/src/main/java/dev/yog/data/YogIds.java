package dev.yog.data;

import dev.yog.YogNames;
import java.util.List;

/** Ids the hosts make of the mods' ones, the same on every version. */
public final class YogIds {
    private YogIds() {}

    /** {@code ns:path}. */
    public static String join(String ns, String path) {
        return ns + YogNames.Sep.NAMESPACE + path;
    }

    /** {@code id}'s path: what follows its namespace (all of it if it has none). */
    public static String path(String id) {
        return id.substring(id.indexOf(YogNames.Sep.NAMESPACE) + 1);
    }

    /** {@code id} in namespace {@code ns} when it has none; else as it is. */
    public static String qualifyIn(String ns, String id) {
        return id.contains(YogNames.Sep.NAMESPACE) ? id : join(ns, id);
    }

    /** The game's {@code id} when it has no namespace; else as it is. */
    public static String qualify(String id) {
        return qualifyIn(YogNames.Ns.MINECRAFT, id);
    }

    /** {@code max_health} → the game's {@code max_health} or {@code generic.max_health}; a full id stays. */
    public static List<String> attributeCandidates(String name) {
        if (name.contains(YogNames.Sep.NAMESPACE)) return List.of(name);
        return List.of(join(YogNames.Ns.MINECRAFT, name), join(YogNames.Ns.MINECRAFT, YogNames.Game.OLD_ATTRIBUTE_PREFIX + name));
    }

    /** Entity type {@code id}'s spawn egg: {@code mymod:wisp_spawn_egg}. */
    public static String spawnEgg(String id) {
        return id + YogNames.Res.SPAWN_EGG_SUFFIX;
    }
}
