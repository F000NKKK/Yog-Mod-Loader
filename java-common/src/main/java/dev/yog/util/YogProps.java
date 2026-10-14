package dev.yog.util;

/**
 * Helpers for the tab-separated lines hosts still exchange with the native
 * runtime (player state, typed command arguments) — content comes as Yog
 * Data ({@code dev.yog.data.YogContent}). Pure Java, no Minecraft types — identical under
 * Yarn and Mojang mappings, so every `YogHost` (Fabric/Forge/NeoForge ×
 * 1.20.1/1.21.1) shares this instead of keeping 6 copies in sync by hand.
 */
public final class YogProps {
    private YogProps() {}

    /** Boolean as a runtime TSV flag: {@code "1"} / {@code "0"}. */
    public static String flag01(boolean b) {
        return b ? "1" : "0";
    }

    /** Make {@code s} safe as one TSV field: {@code null} → {@code ""}, tabs / line breaks → spaces. */
    public static String tsvField(String s) {
        return s == null ? "" : s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }
}
