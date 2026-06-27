package dev.yog;

import com.google.gson.*;
import java.util.*;

/** Lightweight UI layout engine (flexbox-inspired). */
public class YogUILayout {

    public enum FlexDir { ROW, COLUMN }
    public enum Align { START, CENTER, END }

    public static class Style {
        public int w = -1, h = -1, minW, minH, gap, bg, color = 0xFF_FFFFFF;
        public float flex, fontSize = 1.0f;
        public int[] pad = {0,0,0,0}, margin = {0,0,0,0};
        public Align align = Align.START;
    }

    public static class Node {
        public String type, id, text, texture, itemId, onClick;
        public Style style = new Style();
        public List<Node> children = new ArrayList<>();
        public int x, y, w, h;
    }

    public static Node fromJson(String json) { return parseNode(JsonParser.parseString(json).getAsJsonObject()); }

    private static Node parseNode(JsonObject o) {
        Node n = new Node();
        n.type = str(o,"type","panel"); n.id = str(o,"id",null); n.text = str(o,"text",null);
        n.texture = str(o,"texture",null); n.itemId = str(o,"item",null); n.onClick = str(o,"on_click",null);
        if (o.has("style")) parseStyle(o.getAsJsonObject("style"), n.style);
        if (o.has("children")) for (JsonElement c : o.getAsJsonArray("children")) n.children.add(parseNode(c.getAsJsonObject()));
        return n;
    }

    private static void parseStyle(JsonObject o, Style s) {
        if (o.has("width")) s.w=o.get("width").getAsInt(); if (o.has("height")) s.h=o.get("height").getAsInt();
        if (o.has("min_w")) s.minW=o.get("min_w").getAsInt(); if (o.has("min_h")) s.minH=o.get("min_h").getAsInt();
        if (o.has("flex")) s.flex=o.get("flex").getAsFloat(); if (o.has("gap")) s.gap=o.get("gap").getAsInt();
        if (o.has("font_size")) s.fontSize=o.get("font_size").getAsFloat();
        if (o.has("bg")) s.bg=parseHex(o.get("bg").getAsString());
        if (o.has("color")) s.color=parseHex(o.get("color").getAsString());
        if (o.has("align")) s.align=Align.valueOf(o.get("align").getAsString().toUpperCase());
        if (o.has("padding")) s.pad=parseInts(o.getAsJsonArray("padding"),4);
        if (o.has("margin")) s.margin=parseInts(o.getAsJsonArray("margin"),4);
    }

    static int parseHex(String s) { return (int)Long.parseLong(s.startsWith("#")?s.substring(1):s,16); }
    static int[] parseInts(JsonArray a, int len) { int[] r=new int[len]; for(int i=0;i<Math.min(len,a.size());i++) r[i]=a.get(i).getAsInt(); return r; }
    static String str(JsonObject o, String k, String def) { return o.has(k)?o.get(k).getAsString():def; }
}
