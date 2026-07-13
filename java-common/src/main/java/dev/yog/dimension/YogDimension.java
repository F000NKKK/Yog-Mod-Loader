package dev.yog.dimension;

import dev.yog.util.PropertyConverter;

/**
 * A handle to a dimension — either an existing vanilla dimension or a
 * custom one created by a mod.
 *
 * <p>This is the central abstraction of the Yog dimension framework.
 * Everything about a dimension — its type, weather, time, physics, sky,
 * mob spawning — is accessed through {@link #getProperty(String)} and
 * mutated through {@link #setProperty(String, String)}.</p>
 *
 * <p>Property keys use dot-separated namespaced paths,
 * e.g. {@code "weather.can_rain"}, {@code "sky.fog_color"},
 * {@code "spawning.monster_light_threshold"}.</p>
 *
 * <p>To react to changes the Java host broadcasts {@link YogDimensionEvent}s
 * via {@link #addListener(YogDimensionListener)}.</p>
 *
 * <p>To create new dimensions at server startup, mods register
 * {@link YogDimensionDef}s via the NativeBridge.</p>
 */
public interface YogDimension {

    /** Registry id, e.g. {@code "minecraft:overworld"} or {@code "mymod:my_dim"}. */
    String id();

    /// ── Generic property access ────────────────────────────────────────────

    /**
     * Get a property value as a string.
     * @param key dot-separated namespaced path
     * @return value, or {@code null} if the property is not set / not applicable
     */
    String getProperty(String key);

    /**
     * Get a property as a typed value using a converter.
     * @return value, or {@code fallback} if not set
     */
    default <T> T getProperty(String key, PropertyConverter<T> converter, T fallback) {
        String raw = getProperty(key);
        return raw != null ? converter.parse(raw) : fallback;
    }

    /**
     * Set a property value.
     * @return {@code true} if the property was accepted, {@code false} if
     *         the property is unknown or the value was rejected by the host.
     */
    boolean setProperty(String key, String value);

    /** Convenience: set a property from a typed value using a converter. */
    default <T> boolean setProperty(String key, T value, PropertyConverter<T> converter) {
        return setProperty(key, converter.serialize(value));
    }

    /// ── All known property keys (keys known to the host) ───────────────────

    /** Return all property keys this dimension supports. */
    java.util.Set<String> knownPropertyKeys();

    /// ── Events ─────────────────────────────────────────────────────────────

    /** Add a listener for dimension events. */
    void addListener(YogDimensionListener listener);

    /** Remove a listener. */
    void removeListener(YogDimensionListener listener);

    /// ── Lifecycle ──────────────────────────────────────────────────────────

    /** Whether the dimension's world is currently loaded. */
    boolean isLoaded();

    /** Unload this dimension (if it was custom-loaded). No-op for vanilla dimensions. */
    void unload();

    /// ── Type ────────────────────────────────────────────────────────────────

    /** The dimension type properties. */
    YogDimensionType type();

    /// ── Dimension type shortcut properties ──────────────────────────────────

    /** Minimum build height. */
    default int minY() { return intProperty("type.min_y", -64); }
    /** Total height. */
    default int height() { return intProperty("type.height", 384); }
    /** Maximum build height. */
    default int maxY() { return minY() + height() - 1; }

    /// ── Built-in property keys ─────────────────────────────────────────────

    /** Property key prefix for dimension type properties. */
    String TYPE_PREFIX = "type.";
    /** Property key for weather. */
    String WEATHER_PREFIX = "weather.";
    /** Property key for sky/fog. */
    String SKY_PREFIX = "sky.";
    /** Property key for mob spawning. */
    String SPAWNING_PREFIX = "spawning.";
    /** Property key for world border. */
    String BORDER_PREFIX = "border.";
    /** Property key for day/night cycle. */
    String TIME_PREFIX = "time.";
    /** Property key for physics (ultrawarm, beds, etc). */
    String PHYSICS_PREFIX = "physics.";

    // ── Helper ──────────────────────────────────────────────────────────────

    private int intProperty(String key, int fallback) {
        return getProperty(key, PropertyConverter.INTEGER, fallback);
    }
}
