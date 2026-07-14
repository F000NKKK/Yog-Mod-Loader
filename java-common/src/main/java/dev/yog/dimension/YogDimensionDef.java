package dev.yog.dimension;

/**
 * A dimension definition — what a mod declares to create a custom dimension
 * type. This only describes the *type* (sky, lighting, physics, ...); it
 * does not create a world by itself — see {@link YogDimensions#create(String)}.
 *
 * <p>This is a simple data holder. The {@link #extra()} map lets mods
 * attach arbitrary platform/metadata without the framework needing to
 * know about every Minecraft-specific setting.</p>
 *
 * <p>Usage from Rust: the {@code yog-dimensions} crate's
 * {@code YogDimensionDef::to_json()} serializes the full config
 * ({@code id}, {@code dimension_type}, {@code extra}), sent via
 * {@code Registry::register_dimension}, which the Java host parses into
 * this def. Chunk generation is a separate concern — mods register a Rust
 * closure via {@code Registry::register_chunk_generator}, not a config
 * value here (see the package docs).</p>
 *
 * <p>Usage from Java:</p>
 * <pre>{@code
 * YogDimensionDef def = new YogDimensionDef("mymod:my_dim", myType);
 * def.extra().put("note", "anything not modeled by YogDimensionType");
 * YogDimensions.declare(def);
 * // ... later, at any point at runtime:
 * YogDimensions.create("mymod:my_dim");
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