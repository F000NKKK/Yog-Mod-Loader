package dev.yog.util;

/**
 * Two-way converter between a typed value and its string representation.
 *
 * <p>Built-in instances:</p>
 * <pre>{@code
 * PropertyConverter.INTEGER
 * PropertyConverter.FLOAT
 * PropertyConverter.DOUBLE
 * PropertyConverter.BOOLEAN
 * PropertyConverter.STRING
 * PropertyConverter.HEX_COLOR
 * }</pre>
 */
@FunctionalInterface
public interface PropertyConverter<T> {

    /** Parse a string into a value. */
    T parse(String raw);

    /** Serialize a value into a string. Override for custom formatting. */
    default String serialize(T value) {
        return value != null ? value.toString() : "";
    }

    // ── Built-in converters ─────────────────────────────────────────────────

    PropertyConverter<String>  STRING  = raw -> raw;
    PropertyConverter<Integer> INTEGER = raw -> {
        try { return Integer.parseInt(raw); } catch (NumberFormatException e) { return 0; }
    };
    PropertyConverter<Long>    LONG    = raw -> {
        try { return Long.parseLong(raw); } catch (NumberFormatException e) { return 0L; }
    };
    PropertyConverter<Float>   FLOAT   = raw -> {
        try { return Float.parseFloat(raw); } catch (NumberFormatException e) { return 0f; }
    };
    PropertyConverter<Double>  DOUBLE  = raw -> {
        try { return Double.parseDouble(raw); } catch (NumberFormatException e) { return 0.0; }
    };
    PropertyConverter<Boolean> BOOLEAN = raw -> "true".equalsIgnoreCase(raw) || "1".equals(raw);

    /** Converter that stores a hex color (0xRRGGBB) as int and formats as hex string. */
    PropertyConverter<Integer> HEX_COLOR = new PropertyConverter<>() {
        @Override
        public Integer parse(String raw) {
            try {
                return raw.startsWith("0x") || raw.startsWith("0X")
                    ? Integer.parseInt(raw.substring(2), 16)
                    : Integer.parseInt(raw);
            } catch (NumberFormatException e) {
                return null;
            }
        }
        @Override
        public String serialize(Integer value) {
            return value != null ? String.format("0x%06X", value & 0xFFFFFF) : "";
        }
    };
}