package dev.yog.dimension;

/**
 * Builder for creating new {@link YogDimensionType} definitions.
 *
 * <p>All properties have sensible defaults matching the vanilla overworld.
 * Call only the setters you want to override, then build.</p>
 *
 * <pre>{@code
 * YogDimensionType myType = YogDimensionTypeBuilder.create("mymod:my_type")
 *     .ultrawarm(true)
 *     .effects("nether")
 *     .build();
 * }</pre>
 */
public final class YogDimensionTypeBuilder {

    private final String id;
    private int minY = -64;
    private int height = 384;
    private int logicalHeight = 384;
    private boolean hasSkyLight = true;
    private boolean skyLightUpdates = true;
    private float ambientLight = 0.0f;
    private float coordinateScale = 1.0f;
    private boolean ultrawarm = false;
    private boolean bedsExplode = false;
    private boolean respawnAnchorsExplode = false;
    private boolean piglinSafe = false;
    private boolean natural = true;
    private boolean hasCeiling = false;
    private String effects = "overworld";
    private boolean hasSky = true;
    private boolean hasClouds = true;
    private double cloudHeight = 192.0;
    private boolean hasFog = false;
    private Integer fogColor = null;
    private Integer skyColor = null;
    private Integer waterColor = null;
    private Integer waterFogColor = null;

    private YogDimensionTypeBuilder(String id) {
        if (id == null || id.isEmpty()) throw new IllegalArgumentException("id must not be empty");
        this.id = id;
    }

    /** Start building a new dimension type. */
    public static YogDimensionTypeBuilder create(String id) {
        return new YogDimensionTypeBuilder(id);
    }

    /** Copy all properties from an existing type as starting point. */
    public static YogDimensionTypeBuilder from(YogDimensionType other) {
        YogDimensionTypeBuilder b = new YogDimensionTypeBuilder(other.id());
        b.minY = other.minY();
        b.height = other.height();
        b.logicalHeight = other.logicalHeight();
        b.hasSkyLight = other.hasSkyLight();
        b.skyLightUpdates = other.skyLightUpdates();
        b.ambientLight = other.ambientLight();
        b.coordinateScale = other.coordinateScale();
        b.ultrawarm = other.ultrawarm();
        b.bedsExplode = other.bedsExplode();
        b.respawnAnchorsExplode = other.respawnAnchorsExplode();
        b.piglinSafe = other.piglinSafe();
        b.natural = other.natural();
        b.hasCeiling = other.hasCeiling();
        b.effects = other.effects();
        b.hasSky = other.hasSky();
        b.hasClouds = other.hasClouds();
        b.cloudHeight = other.cloudHeight();
        b.hasFog = other.hasFog();
        b.fogColor = other.fogColor();
        b.skyColor = other.skyColor();
        b.waterColor = other.waterColor();
        b.waterFogColor = other.waterFogColor();
        return b;
    }

    // ── Setters ─────────────────────────────────────────────────────────────

    public YogDimensionTypeBuilder minY(int v) { this.minY = v; return this; }
    public YogDimensionTypeBuilder height(int v) { this.height = v; this.logicalHeight = v; return this; }
    public YogDimensionTypeBuilder logicalHeight(int v) { this.logicalHeight = v; return this; }
    public YogDimensionTypeBuilder hasSkyLight(boolean v) { this.hasSkyLight = v; return this; }
    public YogDimensionTypeBuilder skyLightUpdates(boolean v) { this.skyLightUpdates = v; return this; }
    public YogDimensionTypeBuilder ambientLight(float v) { this.ambientLight = v; return this; }
    public YogDimensionTypeBuilder coordinateScale(float v) { this.coordinateScale = v; return this; }
    public YogDimensionTypeBuilder ultrawarm(boolean v) { this.ultrawarm = v; return this; }
    public YogDimensionTypeBuilder bedsExplode(boolean v) { this.bedsExplode = v; return this; }
    public YogDimensionTypeBuilder respawnAnchorsExplode(boolean v) { this.respawnAnchorsExplode = v; return this; }
    public YogDimensionTypeBuilder piglinSafe(boolean v) { this.piglinSafe = v; return this; }
    public YogDimensionTypeBuilder natural(boolean v) { this.natural = v; return this; }
    public YogDimensionTypeBuilder hasCeiling(boolean v) { this.hasCeiling = v; return this; }
    public YogDimensionTypeBuilder effects(String v) { this.effects = v != null ? v : "overworld"; return this; }
    public YogDimensionTypeBuilder hasSky(boolean v) { this.hasSky = v; return this; }
    public YogDimensionTypeBuilder hasClouds(boolean v) { this.hasClouds = v; return this; }
    public YogDimensionTypeBuilder cloudHeight(double v) { this.cloudHeight = v; return this; }
    public YogDimensionTypeBuilder hasFog(boolean v) { this.hasFog = v; return this; }
    public YogDimensionTypeBuilder fogColor(int v) { this.fogColor = v; return this; }
    public YogDimensionTypeBuilder skyColor(int v) { this.skyColor = v; return this; }
    public YogDimensionTypeBuilder waterColor(int v) { this.waterColor = v; return this; }
    public YogDimensionTypeBuilder waterFogColor(int v) { this.waterFogColor = v; return this; }

    /** Build an immutable type descriptor. */
    public YogDimensionType build() {
        return new BuiltType(
            id, minY, height, logicalHeight,
            hasSkyLight, skyLightUpdates, ambientLight,
            coordinateScale,
            ultrawarm, bedsExplode, respawnAnchorsExplode,
            piglinSafe, natural, hasCeiling,
            effects, hasSky, hasClouds, cloudHeight,
            hasFog, fogColor, skyColor, waterColor, waterFogColor
        );
    }

    // ── Built implementation (immutable record) ──────────────────────────────

    private record BuiltType(
        String id,
        int minY,
        int height,
        int logicalHeight,
        boolean hasSkyLight,
        boolean skyLightUpdates,
        float ambientLight,
        float coordinateScale,
        boolean ultrawarm,
        boolean bedsExplode,
        boolean respawnAnchorsExplode,
        boolean piglinSafe,
        boolean natural,
        boolean hasCeiling,
        String effects,
        boolean hasSky,
        boolean hasClouds,
        double cloudHeight,
        boolean hasFog,
        Integer fogColor,
        Integer skyColor,
        Integer waterColor,
        Integer waterFogColor
    ) implements YogDimensionType {}
}