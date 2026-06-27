package dev.yog;

import com.google.gson.*;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.recipe.Recipe;
import net.minecraft.registry.Registries;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import java.util.*;

/** GUI screen that renders a Yog book with categories, entries and pages. */
public class YogBookScreen extends Screen {
    private static final int WIDTH = 292, HEIGHT = 190;
    private static final int LEFT_W = 100, RIGHT_X = 106, RIGHT_W = 186;

    private final String bookId;
    private final JsonObject bookJson;
    private final List<Category> categories = new ArrayList<>();
    private final Map<String, List<Entry>> entriesByCat = new LinkedHashMap<>();
    private final Map<String, Entry> entriesById = new LinkedHashMap<>();
    private int selCat, selEntry, curPage;
    private List<String> curPages = new ArrayList<>();
    private int guiLeft, guiTop;

    public YogBookScreen(String bookId) {
        super(Text.literal("Book"));
        this.bookId = bookId;
        String raw = NativeBridge.nativeBookJson(bookId);
        this.bookJson = (raw != null && !raw.equals("null"))
                ? JsonParser.parseString(raw).getAsJsonObject() : null;
        if (bookJson != null) parseBook();
    }

    @SuppressWarnings("unchecked")
    private <T> T g(JsonObject o, String k, T def) {
        if (!o.has(k)) return def;
        JsonElement e = o.get(k);
        if (def instanceof String) return (T) e.getAsString();
        if (def instanceof Integer) return (T) (Integer) e.getAsInt();
        if (def instanceof Boolean) return (T) (Boolean) e.getAsBoolean();
        return def;
    }

    private void parseBook() {
        for (JsonElement c : bookJson.getAsJsonArray("categories")) {
            JsonObject o = c.getAsJsonObject();
            categories.add(new Category(g(o, "id", ""), g(o, "name", ""),
                    g(o, "description", null), g(o, "icon", null), g(o, "sortnum", 0)));
        }
        categories.sort(Comparator.comparingInt(c -> c.sortnum));
        for (Category cat : categories) entriesByCat.put(cat.id, new ArrayList<>());
        for (JsonElement e : bookJson.getAsJsonArray("entries")) {
            JsonObject o = e.getAsJsonObject();
            Entry en = new Entry(g(o, "id", ""), g(o, "name", ""),
                    g(o, "category", ""), g(o, "icon", null));
            for (JsonElement p : o.getAsJsonArray("pages")) {
                JsonObject po = p.getAsJsonObject();
                String type = g(po, "type", "text");
                PageData pd = new PageData(type);
                pd.text = g(po, "text", null);
                pd.itemId = g(po, "item", null);
                pd.recipeId = g(po, "recipe", null);
                pd.texture = g(po, "texture", null);
                pd.title = g(po, "title", null);
                pd.border = g(po, "border", false);
                en.pages.add(pd);
            }
            entriesByCat.computeIfAbsent(en.cat, k -> new ArrayList<>()).add(en);
            entriesById.put(en.id, en);
        }
    }

    @Override
    protected void init() {
        guiLeft = (width - WIDTH) / 2;
        guiTop = (height - HEIGHT) / 2;
        for (int i = 0; i < categories.size(); i++) {
            final int idx = i;
            addDrawableChild(ButtonWidget.builder(Text.literal(categories.get(i).name),
                    btn -> { selCat = idx; selEntry = 0; curPage = 0; clearChildren(); init(); })
                    .dimensions(guiLeft + 2, guiTop + 14 + i * 14, LEFT_W - 4, 12).build());
        }
        addDrawableChild(ButtonWidget.builder(Text.literal("<"),
                btn -> { if (curPage > 0) curPage--; })
                .dimensions(guiLeft + RIGHT_X, guiTop + HEIGHT - 16, 20, 14).build());
        addDrawableChild(ButtonWidget.builder(Text.literal(">"),
                btn -> { if (curPage < curPages.size() - 1) curPage++; })
                .dimensions(guiLeft + RIGHT_X + RIGHT_W - 20, guiTop + HEIGHT - 16, 20, 14).build());
        rebuildEntries();
    }

    private void rebuildEntries() {
        List<Entry> list = entriesByCat.getOrDefault(
                categories.isEmpty() ? "" : categories.get(selCat).id, Collections.emptyList());
        for (int i = 0; i < list.size(); i++) {
            final int idx = i;
            String label = list.get(i).name;
            if (label.length() > 11) label = label.substring(0, 11);
            addDrawableChild(ButtonWidget.builder(Text.literal(label), btn -> {
                selEntry = idx; curPage = 0; buildPages(list.get(idx)); })
                    .dimensions(guiLeft + 2, guiTop + 50 + i * 12, LEFT_W - 4, 11).build());
        }
        if (!list.isEmpty()) buildPages(list.get(0));
    }

    private void buildPages(Entry en) {
        curPages.clear();
        for (PageData pd : en.pages) {
            switch (pd.type) {
                case "text": curPages.add("T|" + (pd.text != null ? pd.text : "")); break;
                case "spotlight": curPages.add("S|" + pd.itemId + "|" + pd.title + "|" + pd.text); break;
                case "crafting": curPages.add("C|" + pd.recipeId + "|" + pd.text); break;
                case "smelting": curPages.add("M|" + pd.recipeId + "|" + pd.text); break;
                case "empty": curPages.add("E|"); break;
                default: curPages.add("T|" + pd.type); break;
            }
        }

    }

    @Override
    public void render(DrawContext ctx, int mx, int my, float delta) {
        renderBackground(ctx);
        super.render(ctx, mx, my, delta);
        int x = guiLeft, y = guiTop;
        ctx.fill(x, y, x + WIDTH, y + HEIGHT, 0xFF_2A1A0E);
        ctx.fill(x + 1, y + 1, x + WIDTH - 1, y + HEIGHT - 1, 0xFF_5C3A1E);
        ctx.fill(x + LEFT_W, y + 4, x + LEFT_W + 2, y + HEIGHT - 4, 0xFF_8B6914);
        String title = bookJson != null && bookJson.has("name")
                ? bookJson.get("name").getAsString() : bookId;
        ctx.drawCenteredTextWithShadow(textRenderer, title, x + WIDTH / 2, y + 2, 0xFF_D4A84B);
        ctx.drawTextWithShadow(textRenderer, "Categories", x + 4, y + 4, 0xFF_888888);
        if (!curPages.isEmpty() && curPage < curPages.size())
            renderPage(ctx, curPages.get(curPage), x + RIGHT_X + 4, y + 28);
        if (!curPages.isEmpty()) {
            String num = (curPage + 1) + "/" + curPages.size();
            ctx.drawTextWithShadow(textRenderer, num,
                    x + RIGHT_X + RIGHT_W / 2 - textRenderer.getWidth(num) / 2,
                    y + HEIGHT - 14, 0xFF_888888);
        }
    }

    private void renderPage(DrawContext ctx, String page, int x, int y) {
        String[] p = page.split("\\|", 4);
        if ("T".equals(p[0]) || "E".equals(p[0])) {
            drawWrapped(ctx, p.length > 1 ? p[1] : "", x, y);
        } else if ("S".equals(p[0])) {
            String itemId = p.length > 1 ? p[1] : "";
            String title = p.length > 2 && !"null".equals(p[2]) ? p[2] : null;
            String text = p.length > 3 && !"null".equals(p[3]) ? p[3] : "";
            if (title != null) ctx.drawTextWithShadow(textRenderer, title, x + 20, y, 0xFF_FFFF55);
            Identifier id = Identifier.tryParse(itemId);
            if (id != null && Registries.ITEM.containsId(id))
                ctx.drawItem(new ItemStack(Registries.ITEM.get(id)), x, y + 8);
            drawWrapped(ctx, text, x + 20, y + 22);
        } else if ("C".equals(p[0]) || "M".equals(p[0])) {
            String rid = p.length > 1 ? p[1] : "";
            String text = p.length > 2 && !"null".equals(p[2]) ? p[2] : "";
            drawWrapped(ctx, text, x, y + 56);
        }
    }

    private void drawWrapped(DrawContext ctx, String text, int x, int y) {
        for (String para : text.split("\\\\n"))
            for (String raw : para.split("\n")) {
                String line = raw;
                while (!line.isEmpty()) {
                    if (textRenderer.getWidth(line) <= RIGHT_W - 8) {
                        if (y <= guiTop + HEIGHT - 16)
                            ctx.drawTextWithShadow(textRenderer, line, x, y, 0xFF_CCCCAA);
                        y += 10; break;
                    }
                    int cut = line.length();
                    while (cut > 0 && textRenderer.getWidth(line.substring(0, cut)) > RIGHT_W - 8) cut--;
                    if (cut == 0) cut = 1;
                    if (y <= guiTop + HEIGHT - 16)
                        ctx.drawTextWithShadow(textRenderer, line.substring(0, cut), x, y, 0xFF_CCCCAA);
                    y += 10; line = line.substring(cut);
                }
            }
    }

    @Override public boolean shouldPause() { return false; }

    record Category(String id, String name, String desc, String icon, int sortnum) {}
    static class Entry {
        final String id, name, cat, icon;
        final List<PageData> pages = new ArrayList<>();
        Entry(String id, String name, String cat, String icon) {
            this.id = id; this.name = name; this.cat = cat; this.icon = icon;
        }
    }
    static class PageData { String type, text, itemId, recipeId, texture, title; boolean border; PageData(String t) { type = t; } }
}