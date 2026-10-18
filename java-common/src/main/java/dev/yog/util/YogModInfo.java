package dev.yog.util;

/** A Java mod the loader has loaded, as the runtime's mod list shows it. */
public record YogModInfo(String id, String name, String version, String authors, String description) {}
