package dev.yog.dimension;

/**
 * A dimension definition — what a mod registers to create a custom dimension.
 *
 * <p>This is a simple data holder. The {@link #extra()} map lets mods
 * attach arbitrary platform/metadata without the framework needing to
 * know about every Minecraft-specific setting.</p>
 *
 * <p>Usage from Rust: the {@code yog-dimensions} crate serializes
 * the full config as JSON, which the Java host parses into this def.</p>
 *
 * <p>Usage from Java:</p>
 * <pre>{@code
 * YogDimensionDef def = new YogDimensionDef("mymod:my_dim", myType);
 * def.extra().put("generator", "{\"type\":\"noise\",\"preset\":\"minecraft:overworld\"}");
 * YogDimensions.register(def);
 * }</pre>
 */
public final class YogDimensionDef {

    private final String id;
    private final YogDimensionType type;
    private final java.util.Map<String, String> extra;

    public YogDimensionDef(String id, YogDimensionType type) {
        if (id == null || id.isEmpty()) throw new IllegalArgumentException("id must not be empty");
        if (type == null) throw new IllegalArgumentException("type must not be null");
        this.id = id;
        this.type = type;
        this.extra = new java.util.HashMap<>();
    }

    /** Unique registry id, e.g. {@code "mymod:my_dim"}. */
    public String id() { return id; }

    /** The dimension type properties. */
    public YogDimensionType type() { return type; }

    /**
     * Extra metadata — any platform-specific configuration the host needs
     * to create this dimension: chunk generator config, spawn rules, etc.
     *
     * <p>Keys are dot-separated paths like the property system.
     * Values are JSON strings.</p>
     */
    public java.util.Map<String, String> extra() { return extra; }

    /** Convenience: set an extra property. */
    public YogDimensionDef withExtra(String key, String value) {
        extra.put(key, value);
        return this;
    }
}