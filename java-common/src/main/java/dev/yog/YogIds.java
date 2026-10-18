package dev.yog;

/**
 * The namespaces and id forms Yog's content uses, in one place for every
 * host: the version-neutral side of ids (strings); each host wraps the
 * results in its game version's id type.
 */
public final class YogIds {
    private YogIds() {}

    /** The game's namespace. */
    public static final String MINECRAFT = "minecraft";
    /** Yog's own namespace: its tags and its stable aliases of the game's content. */
    public static final String YOG = "yog";
    /** Yog's alias of one of the game's potions: {@code yog:potion/<name>}. */
    public static final String POTION_ALIAS = YOG + ":potion/";
    private static final String YOG_PREFIX = YOG + ":";

    /** The translation key of attribute {@code id}'s name: {@code attribute.name.<ns>.<path>}. */
    public static String attributeNameKey(String id) {
        int colon = id.indexOf(':');
        String ns = colon < 0 ? MINECRAFT : id.substring(0, colon);
        String path = id.substring(colon + 1);
        return "attribute.name." + ns + "." + path;
    }

    /** Whether {@code id} is Yog's alias of one of the game's potions. */
    public static boolean isPotionAlias(String id) {
        return id.startsWith(POTION_ALIAS);
    }

    /**
     * The game's id of the potion {@code id} names: an alias
     * ({@code yog:potion/awkward}, {@code yog:awkward}) is the game's
     * ({@code minecraft:awkward}); a mod's id is itself.
     */
    public static String gamePotionId(String id) {
        if (isPotionAlias(id)) return MINECRAFT + ":" + id.substring(POTION_ALIAS.length());
        if (id.startsWith(YOG_PREFIX)) return MINECRAFT + ":" + id.substring(YOG_PREFIX.length());
        return id;
    }
}
