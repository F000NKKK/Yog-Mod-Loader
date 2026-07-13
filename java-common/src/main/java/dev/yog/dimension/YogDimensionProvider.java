package dev.yog.dimension;

/**
 * Platform-specific provider that connects the Yog dimension framework
 * to the underlying Minecraft server.
 *
 * <p>Each platform (Fabric, Forge, NeoForge) provides its own implementation
 * and installs it via {@link YogDimensions#install(YogDimensionProvider)}
 * at server startup.</p>
 */
public interface YogDimensionProvider {

    /** Get a dimension handle by its registry id. */
    YogDimension getDimension(String id);

    /** Get a dimension type by its registry id. */
    YogDimensionType getDimensionType(String id);

    /** All currently loaded/registered dimensions. */
    java.util.Collection<YogDimension> allDimensions();

    /** Register a custom dimension definition to be created at server startup. */
    void register(YogDimensionDef def);
}