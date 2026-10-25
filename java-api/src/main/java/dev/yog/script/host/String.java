package dev.yog.script.host;

import dev.yog.script.collections.generic.IEnumerable;
import dev.yog.script.collections.generic.List;

/**
 * {@code string}'s members as Yog Script has them ({@code System.String},
 * .NET's): a static each, the string first ({@code s.ToUpper()} is
 * {@code String.ToUpper(s)}). Java's own string is written in full here:
 * this class's name is the same.
 */
public final class String {
    /** No text. */
    public static final java.lang.String Empty = "";

    private static final int NOT_FOUND = -1;

    private String() {}

    public static int Length(java.lang.String s) {
        return s.length();
    }

    public static char get_Item(java.lang.String s, int index) {
        return s.charAt(index);
    }

    public static java.lang.String Substring(java.lang.String s, int startIndex) {
        return s.substring(startIndex);
    }

    /** {@code length} characters from {@code startIndex} (C#'s, not Java's end index). */
    public static java.lang.String Substring(java.lang.String s, int startIndex, int length) {
        return s.substring(startIndex, startIndex + length);
    }

    public static java.lang.String ToUpper(java.lang.String s) {
        return s.toUpperCase(java.util.Locale.ROOT);
    }

    public static java.lang.String ToLower(java.lang.String s) {
        return s.toLowerCase(java.util.Locale.ROOT);
    }

    public static java.lang.String Trim(java.lang.String s) {
        return s.strip();
    }

    public static java.lang.String TrimStart(java.lang.String s) {
        return s.stripLeading();
    }

    public static java.lang.String TrimEnd(java.lang.String s) {
        return s.stripTrailing();
    }

    public static java.lang.String PadLeft(java.lang.String s, int totalWidth) {
        return " ".repeat(Math.max(0, totalWidth - s.length())) + s;
    }

    public static java.lang.String PadRight(java.lang.String s, int totalWidth) {
        return s + " ".repeat(Math.max(0, totalWidth - s.length()));
    }

    public static java.lang.String Replace(java.lang.String s, java.lang.String oldValue, java.lang.String newValue) {
        return s.replace(oldValue, newValue);
    }

    public static java.lang.String Insert(java.lang.String s, int startIndex, java.lang.String value) {
        return s.substring(0, startIndex) + value + s.substring(startIndex);
    }

    public static java.lang.String Remove(java.lang.String s, int startIndex, int count) {
        return s.substring(0, startIndex) + s.substring(startIndex + count);
    }

    public static boolean StartsWith(java.lang.String s, java.lang.String value) {
        return s.startsWith(value);
    }

    public static boolean EndsWith(java.lang.String s, java.lang.String value) {
        return s.endsWith(value);
    }

    public static boolean Contains(java.lang.String s, java.lang.String value) {
        return s.contains(value);
    }

    public static int IndexOf(java.lang.String s, java.lang.String value) {
        return s.indexOf(value);
    }

    public static int IndexOf(java.lang.String s, char value) {
        return s.indexOf(value);
    }

    public static int LastIndexOf(java.lang.String s, java.lang.String value) {
        return s.lastIndexOf(value);
    }

    public static int LastIndexOf(java.lang.String s, char value) {
        return s.lastIndexOf(value);
    }

    public static java.lang.String[] Split(java.lang.String s, char separator) {
        return Split(s, java.lang.String.valueOf(separator));
    }

    /** The parts between each {@code separator}, empty ones kept (C#'s, not Java's regex split). */
    public static java.lang.String[] Split(java.lang.String s, java.lang.String separator) {
        List<java.lang.String> parts = new List<>();
        int from = 0;
        int at = s.indexOf(separator);
        while (at != NOT_FOUND && !separator.isEmpty()) {
            parts.Add(s.substring(from, at));
            from = at + separator.length();
            at = s.indexOf(separator, from);
        }
        parts.Add(s.substring(from));
        return parts.toArray(new java.lang.String[0]);
    }

    public static boolean Equals(java.lang.String s, java.lang.String value) {
        return s.equals(value);
    }

    public static int CompareTo(java.lang.String s, java.lang.String value) {
        return s.compareTo(value);
    }

    public static boolean IsNullOrEmpty(java.lang.String value) {
        return value == null || value.isEmpty();
    }

    public static boolean IsNullOrWhiteSpace(java.lang.String value) {
        return value == null || value.isBlank();
    }

    public static java.lang.String Join(java.lang.String separator, IEnumerable<java.lang.String> values) {
        return java.lang.String.join(separator, values);
    }

    public static java.lang.String Concat(java.lang.String a, java.lang.String b) {
        return a + b;
    }

    public static int Compare(java.lang.String a, java.lang.String b) {
        return a.compareTo(b);
    }
}
