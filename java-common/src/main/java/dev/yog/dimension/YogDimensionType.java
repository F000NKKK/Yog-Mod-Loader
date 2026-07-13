package dev.yog.dimension;

/**
 * Properties of a dimension type — the "what" of a dimension
 * (sky, lighting, physics, coordinate scale, etc.).
 *
 * <p>Each {@link YogDimension} has a type. Multiple dimensions can
 * share the same type (e.g. mod adds 3 worlds with the same sky/weather
 * rules but different terrain generators).</p>
 *
 * <p>This is a read-only snapshot. To create a new dimension type,
 * use {@link YogDimensionTypeBuilder}.</p>
 */
public interface YogDimensionType {

    /** Registry id, e.g. {@code "minecraft:overworld"} or {@code "mymod:my_type"}. */
    String id();

    // ── World geometry ─────────────────────────────────────────────────────

    /** Minimum Y. */
    int minY();

    /** Total height. */
    int height();

    /** Logical height (for nether roof / portal logic). */
    int logicalHeight();

    // ── Lighting ───────────────────────────────────────────────────────────

    /** Whether skylight reaches the ground. */
    boolean hasSkyLight();

    /** Whether skylight updates naturally. */
    boolean skyLightUpdates();

    /** Ambient light level (0.0–1.0). */
    float ambientLight();

    // ── Coordinate scale ───────────────────────────────────────────────────

    /** Coordinate scale for nether portals (vanilla nether: 8.0). */
    float coordinateScale();

    // ── Physics ────────────────────────────────────────────────────────────

    /** Whether water evaporates (nether-like). */
    boolean ultrawarm();

    /** Whether beds explode. */
    boolean bedsExplode();

    /** Whether respawn anchors explode. */
    boolean respawnAnchorsExplode();

    /** Whether piglins are safe here. */
    boolean piglinSafe();

    /** Whether the natural compass works. */
    boolean natural();

    /** Whether there's a ceiling (nether-like). */
    boolean hasCeiling();

    // ── Effects ────────────────────────────────────────────────────────────

    /** Effect type: "overworld", "nether", "end", or a custom id. */
    String effects();

    /** Whether the sky renders. */
    boolean hasSky();

    /** Whether clouds render. */
    boolean hasClouds();

    /** Cloud height. */
    double cloudHeight();

    /** Whether fog is present. */
    boolean hasFog();

    /** Fog color as ARGB, or null for default. */
    Integer fogColor();

    /** Sky color as ARGB, or null for default. */
    Integer skyColor();

    /** Water color as ARGB, or null for default. */
    Integer waterColor();

    /** Water fog color as ARGB, or null for default. */
    Integer waterFogColor();
}