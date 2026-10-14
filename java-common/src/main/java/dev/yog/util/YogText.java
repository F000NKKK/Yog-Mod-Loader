package dev.yog.util;

import java.util.ArrayList;
import java.util.List;

/**
 * Yog rich text (see the yog-ui crate's {@code rich} module): {@code {b|…}},
 * {@code {i|…}}, {@code {u|…}}, {@code {s|…}}, {@code {#rrggbb|…}}, several
 * attributes in one span, spans nested, {@code \{ \} \| \\} escapes — split
 * into runs of one style, which each host turns into its version's text
 * components. Minecraft's own {@code §} codes stay in the text as the game
 * reads them. Mapping-agnostic.
 */
public final class YogText {
    private YogText() {}

    /** One stretch of text in one style; {@code rgb} {@code -1}: the text's own color. */
    public record Run(String text, int rgb, boolean bold, boolean italic, boolean underline, boolean strike) {}

    private record Style(int rgb, boolean bold, boolean italic, boolean underline, boolean strike) {
        static final Style NONE = new Style(-1, false, false, false, false);
    }

    /** Whether {@code s} may hold spans (the plain path otherwise). */
    public static boolean rich(String s) {
        return s.indexOf('{') >= 0 || s.indexOf('\\') >= 0;
    }

    /** {@code attrs} applied to {@code st}; {@code null} if they aren't span attributes. */
    private static Style apply(String attrs, Style st) {
        String[] parts = attrs.trim().split("\\s+");
        if (attrs.isBlank()) return null;
        int rgb = st.rgb();
        boolean b = st.bold(), i = st.italic(), u = st.underline(), s = st.strike();
        for (String a : parts) {
            switch (a) {
                case "b" -> b = true;
                case "i" -> i = true;
                case "u" -> u = true;
                case "s" -> s = true;
                default -> {
                    if (a.length() != 7 || a.charAt(0) != '#') return null;
                    try {
                        rgb = Integer.parseInt(a.substring(1), 16);
                    } catch (NumberFormatException e) {
                        return null;
                    }
                }
            }
        }
        return new Style(rgb, b, i, u, s);
    }

    public static List<Run> runs(String s) {
        List<Run> out = new ArrayList<>();
        List<Style> stack = new ArrayList<>();
        stack.add(Style.NONE);
        StringBuilder cur = new StringBuilder();
        Style curStyle = Style.NONE;
        int n = s.length();
        for (int k = 0; k < n; k++) {
            char c = s.charAt(k);
            Style top = stack.get(stack.size() - 1);
            if (c == '\\' && k + 1 < n) {
                if (!top.equals(curStyle)) { flush(out, cur, curStyle); curStyle = top; }
                cur.append(s.charAt(++k));
                continue;
            }
            if (c == '{') {
                int bar = -1;
                for (int j = k + 1; j < n; j++) {
                    char d = s.charAt(j);
                    if (d == '|') { bar = j; break; }
                    if (d == '{' || d == '}') break;
                }
                Style opened = bar > 0 ? apply(s.substring(k + 1, bar), top) : null;
                if (opened != null) {
                    stack.add(opened);
                    k = bar;
                    continue;
                }
            }
            if (c == '}' && stack.size() > 1) {
                stack.remove(stack.size() - 1);
                continue;
            }
            if (!top.equals(curStyle)) { flush(out, cur, curStyle); curStyle = top; }
            cur.append(c);
        }
        flush(out, cur, curStyle);
        return out;
    }

    private static void flush(List<Run> out, StringBuilder cur, Style st) {
        if (cur.length() == 0) return;
        out.add(new Run(cur.toString(), st.rgb(), st.bold(), st.italic(), st.underline(), st.strike()));
        cur.setLength(0);
    }
}
