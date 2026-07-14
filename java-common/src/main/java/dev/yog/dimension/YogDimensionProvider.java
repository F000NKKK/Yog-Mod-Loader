package dev.yog.dimension;

/**
 * Platform-specific provider that connects the Yog dimension framework
 * to the underlying Minecraft server.
 *
 * <p>Each platform (Fabric, Forge, NeoForge) provides its own implementation
 * and installs it via {@link YogDimensions#install(YogDimensionProvider)}
 * at server startup.</p>
 *
 * <p>Two-phase lifecycle — see the package docs:
 * {@link #declare(YogDimensionDef)} happens once, at mod-init time (before
 * Minecraft's registries freeze); {@link #create(String)} can happen at
 * any point while the server is running, with no restart required.</p>
 */
public interface YogDimensionProvider {

    /** Get a dimension handle by its registry id, or {@code null} if not created yet. */
    YogDimension getDimension(String id);

    /** Get a dimension type by its registry id. */
    YogDimensionType getDimensionType(String id);

    /** All currently loaded (created) dimensions. */
    java.util.Collection<YogDimension> allDimensions();

    /**
     * Declare a custom dimension's type. Must be called at mod-init time
     * (same window as block/item registration) so the type is known to
     * every connecting client's login-time registry sync. Declaring a
     * type does not create a world — see {@link #create(String)}.
     */
    void declare(YogDimensionDef def);

    /**
     * Materialize the actual world for a previously-{@link #declare}d
     * dimension id — callable at any point while the server is running,
     * no restart required. Returns the existing handle if already created.
     */
    YogDimension create(String id);
}