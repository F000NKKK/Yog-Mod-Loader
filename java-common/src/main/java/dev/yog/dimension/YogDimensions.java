package dev.yog.dimension;

/**
 * Entry point for accessing dimensions in the Yog framework.
 *
 * <p>Provides access to built-in vanilla dimensions and the set of
 * all registered dimensions (both vanilla and custom).</p>
 *
 * <p>Implementation is provided by the platform host (Fabric/Forge/NeoForge)
 * at server startup via {@link #install(YogDimensionProvider)}.</p>
 */
public final class YogDimensions {

    private static YogDimensionProvider provider;

    private YogDimensions() {}

    /** Install the platform provider. Called once at server start. */
    public static void install(YogDimensionProvider p) {
        provider = p;
    }

    private static YogDimensionProvider provider() {
        if (provider == null) throw new IllegalStateException("YogDimensions not installed (server not started?)");
        return provider;
    }

    /** Get a dimension by its registry id. */
    public static YogDimension get(String id) {
        return provider().getDimension(id);
    }

    /** The overworld. */
    public static YogDimension overworld() {
        return get("minecraft:overworld");
    }

    /** The nether. */
    public static YogDimension nether() {
        return get("minecraft:the_nether");
    }

    /** The end. */
    public static YogDimension end() {
        return get("minecraft:the_end");
    }

    /** All currently loaded/registered dimensions. */
    public static java.util.Collection<YogDimension> all() {
        return provider().allDimensions();
    }

    /**
     * Declare a custom dimension's type at mod-init time — see
     * {@link YogDimensionProvider#declare(YogDimensionDef)}. Does not create
     * a world by itself; call {@link #create(String)} whenever the world
     * should actually come into existence.
     */
    public static void declare(YogDimensionDef def) {
        provider().declare(def);
    }

    /**
     * Materialize the world for a previously-{@link #declare}d dimension id.
     * Safe to call at any point while the server is running — see
     * {@link YogDimensionProvider#create(String)}.
     */
    public static YogDimension create(String id) {
        return provider().create(id);
    }

    /** Get a dimension type by its registry id. */
    public static YogDimensionType type(String id) {
        return provider().getDimensionType(id);
    }
}