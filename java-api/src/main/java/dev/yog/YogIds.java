package dev.yog;

/**
 * Working with ids — the version-neutral side (strings) of what every host
 * does with them; the names themselves are {@link YogNames} (generated from
 * yog-ids). Each host wraps the results in its game version's id type.
 */
public final class YogIds {
    private YogIds() {}

    private static final String YOG_PREFIX = YogNames.Ns.YOG + YogNames.Sep.NAMESPACE;
    private static final String ATTRIBUTE_NAME_KEY = "attribute.name.";

    /** {@code id}'s namespace; a bare path is the game's. */
    public static String namespace(String id) {
        int sep = id.indexOf(YogNames.Sep.NAMESPACE);
        return sep < 0 ? YogNames.Ns.MINECRAFT : id.substring(0, sep);
    }

    /** {@code id}'s path. */
    public static String path(String id) {
        return id.substring(id.indexOf(YogNames.Sep.NAMESPACE) + 1);
    }

    /** {@code ns:path}. */
    public static String join(String ns, String path) {
        return ns + YogNames.Sep.NAMESPACE + path;
    }

    /** The translation key of attribute {@code id}'s name: {@code attribute.name.<ns>.<path>}. */
    public static String attributeNameKey(String id) {
        return ATTRIBUTE_NAME_KEY + namespace(id) + "." + path(id);
    }

    /** Whether {@code font} is the game's default font (an empty one is). */
    public static boolean isDefaultFont(String font) {
        return font == null || font.isEmpty() || font.equals(YogNames.Game.DEFAULT_FONT);
    }

    /** Whether {@code id} is Yog's alias of one of the game's potions. */
    public static boolean isPotionAlias(String id) {
        return id.startsWith(YogNames.Yog.POTION_ALIAS);
    }

    /**
     * The game's id of the potion {@code id} names: an alias
     * ({@code yog:potion/awkward}, {@code yog:awkward}) is the game's
     * ({@code minecraft:awkward}); a mod's id is itself.
     */
    public static String gamePotionId(String id) {
        if (isPotionAlias(id)) return join(YogNames.Ns.MINECRAFT, id.substring(YogNames.Yog.POTION_ALIAS.length()));
        if (id.startsWith(YOG_PREFIX)) return join(YogNames.Ns.MINECRAFT, id.substring(YOG_PREFIX.length()));
        return id;
    }
}
