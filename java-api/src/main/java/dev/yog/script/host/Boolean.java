package dev.yog.script.host;

/**
 * A truth's members: Yog Script's {@code System.Boolean}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Boolean {
    public static final java.lang.String TrueString = "True";
    public static final java.lang.String FalseString = "False";

    private Boolean() {}

    /** {@code True} or {@code False}, any case, around spaces (.NET's); a panic else. */
    public static boolean Parse(java.lang.String s) {
        java.lang.String text = s.strip();
        if (text.equalsIgnoreCase(TrueString)) {
            return true;
        }
        if (text.equalsIgnoreCase(FalseString)) {
            return false;
        }
        throw new java.lang.IllegalArgumentException("`" + s + "` isn't a bool");
    }

    public static java.lang.Boolean TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(boolean value, boolean other) {
        return java.lang.Boolean.compare(value, other);
    }

    public static boolean Equals(boolean value, boolean obj) {
        return value == obj;
    }

    /** {@code True} or {@code False}, as .NET writes them. */
    public static java.lang.String ToString(boolean value) {
        return value ? TrueString : FalseString;
    }
}
