package dev.yog.script.host;

/**
 * A character's members: Yog Script's {@code System.Char}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Char {
    public static final char MinValue = java.lang.Character.MIN_VALUE;
    public static final char MaxValue = java.lang.Character.MAX_VALUE;

    private Char() {}

    public static boolean IsDigit(char c) {
        return java.lang.Character.isDigit(c);
    }

    public static boolean IsLetter(char c) {
        return java.lang.Character.isLetter(c);
    }

    public static boolean IsLetterOrDigit(char c) {
        return java.lang.Character.isLetterOrDigit(c);
    }

    public static boolean IsWhiteSpace(char c) {
        return java.lang.Character.isWhitespace(c) || java.lang.Character.isSpaceChar(c);
    }

    public static boolean IsUpper(char c) {
        return java.lang.Character.isUpperCase(c);
    }

    public static boolean IsLower(char c) {
        return java.lang.Character.isLowerCase(c);
    }

    public static boolean IsPunctuation(char c) {
        return switch (java.lang.Character.getType(c)) {
            case java.lang.Character.CONNECTOR_PUNCTUATION, java.lang.Character.DASH_PUNCTUATION,
                    java.lang.Character.START_PUNCTUATION, java.lang.Character.END_PUNCTUATION,
                    java.lang.Character.INITIAL_QUOTE_PUNCTUATION, java.lang.Character.FINAL_QUOTE_PUNCTUATION,
                    java.lang.Character.OTHER_PUNCTUATION -> true;
            default -> false;
        };
    }

    public static boolean IsControl(char c) {
        return java.lang.Character.isISOControl(c);
    }

    public static char ToUpper(char c) {
        return java.lang.Character.toUpperCase(c);
    }

    public static char ToLower(char c) {
        return java.lang.Character.toLowerCase(c);
    }

    public static double GetNumericValue(char c) {
        return java.lang.Character.getNumericValue(c);
    }

    /** The one character {@code s} is; a panic else. */
    public static char Parse(java.lang.String s) {
        if (s.length() != 1) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't one char");
        }
        return s.charAt(0);
    }

    public static java.lang.Character TryParse(java.lang.String s) {
        return s.length() == 1 ? s.charAt(0) : null;
    }

    public static int CompareTo(char value, char other) {
        return java.lang.Character.compare(value, other);
    }

    public static boolean Equals(char value, char obj) {
        return value == obj;
    }

    public static java.lang.String ToString(char value) {
        return java.lang.String.valueOf(value);
    }
}
