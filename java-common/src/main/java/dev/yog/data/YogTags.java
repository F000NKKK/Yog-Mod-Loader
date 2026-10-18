package dev.yog.data;

import dev.yog.YogLog;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import dev.yog.util.YogDataLayout;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * The tags of every Minecraft version and loader Yog supports, and Yog's
 * common tags over them — {@code yog/tags.txt}, the same table
 * {@code yog build} checks with.
 *
 * <p>{@code yog:<path>} is the game's and the loaders' tags of that meaning
 * ({@code yog:ingots/iron}: {@code c:ingots/iron}, {@code c:iron_ingots},
 * {@code forge:ingots/iron}) — on each host, those it has. What mods put in
 * a {@code yog:} tag goes to the first of them here, so Java mods see it in
 * their tags too; the {@code yog:} tag itself only names them (never the
 * other way round: tags naming each other would be a cycle).
 */
public final class YogTags {
    private YogTags() {}

    /** This host: {@code <loader>/<version>} — set when the pack is staged. */
    private static volatile String host = "";

    private static final List<String> LOADERS = new ArrayList<>();
    /** {@code <registry> <id>} → where it is: {@code *}, versions, {@code <loader>/<version>}s. */
    private static final Map<String, Set<String>> TAGS = new HashMap<>();
    /** {@code <registry> <path>} → the tags {@code yog:<path>} is, the game's first. */
    private static final Map<String, List<String>> YOG = new HashMap<>();
    /** {@code <registry> <path>} → the game's content {@code yog:<path>} holds on every host. */
    private static final Map<String, List<String>> BASE = new HashMap<>();

    static {
        try (InputStream in = YogTags.class.getResourceAsStream("/yog/tags.txt")) {
            if (in != null) {
                BufferedReader r = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8));
                for (String line; (line = r.readLine()) != null; ) {
                    if (line.startsWith("# loaders:")) {
                        LOADERS.addAll(List.of(line.substring("# loaders:".length()).trim().split("\\s+")));
                        continue;
                    }
                    String[] p = line.trim().split("\\s+");
                    if (p.length == 4 && p[0].equals("tag")) {
                        TAGS.put(p[1] + " " + p[2], Set.of(p[3].split(",")));
                    } else if (p.length >= 4 && p[0].equals("yog")) {
                        YOG.put(p[1] + " " + p[2], List.of(p).subList(3, p.length));
                    } else if (p.length >= 4 && p[0].equals("base")) {
                        BASE.put(p[1] + " " + p[2], List.of(p).subList(3, p.length));
                    }
                }
            }
        } catch (Exception e) {
            YogLog.warn("yog/tags.txt: " + e);
        }
    }

    /** This host's tags are the ones to answer for. */
    public static void setHost(String loader, String mcVersion) {
        host = loader + "/" + mcVersion;
    }

    private static boolean here(Set<String> where) {
        if (where.contains("*") || where.contains(host)) return true;
        int slash = host.indexOf('/');
        return slash > 0 && where.contains(host.substring(slash + 1));
    }

    /**
     * Whether tag {@code id} of {@code registry} ({@code item}, {@code block},
     * {@code entity_type}, …) is on this host. Tags the table doesn't know
     * (a mod's own, a datapack's) are taken to be.
     */
    public static boolean has(String registry, String id) {
        if (id.startsWith("yog:")) {
            String key = registry + " " + id.substring(4);
            List<String> of = YOG.get(key);
            return of == null || BASE.containsKey(key) || of.stream().anyMatch(t -> has(registry, t));
        }
        Set<String> where = TAGS.get(registry + " " + id);
        return where == null || here(where);
    }

    /** The tags {@code yog:<path>} is on this host, where its mods' entries go first. */
    static List<String> tagsHere(String registry, String path) {
        List<String> out = new ArrayList<>();
        for (String t : YOG.getOrDefault(registry + " " + path, List.of())) {
            if (has(registry, t)) out.add(t);
        }
        return out;
    }

    /**
     * After the mods' data is staged into {@code pack}: what they put in
     * {@code yog:} tags moves to the tag it is here, and every {@code yog:}
     * tag is written — the tags it is, each optional.
     */
    public static void stage(Path pack, boolean mc121) throws IOException {
        Gson gson = new Gson();
        for (Map.Entry<String, List<String>> e : YOG.entrySet()) {
            String[] key = e.getKey().split(" ", 2);
            String registry = key[0], path = key[1];
            List<String> tags = tagsHere(registry, path);
            Path file = tagFile(pack, registry, "yog:" + path, mc121);
            // The mods' entries, to the first tag it is here.
            Set<JsonElement> entries = new LinkedHashSet<>();
            if (Files.exists(file)) {
                JsonObject o = gson.fromJson(Files.readString(file), JsonObject.class);
                if (o != null && o.has("values")) o.getAsJsonArray("values").forEach(entries::add);
            }
            if (!entries.isEmpty() && !tags.isEmpty()) {
                Path home = tagFile(pack, registry, tags.get(0), mc121);
                JsonArray values = new JsonArray();
                entries.forEach(values::add);
                JsonObject add = new JsonObject();
                add.add("values", values);
                Files.createDirectories(home.getParent());
                String json = gson.toJson(add);
                Files.writeString(home, Files.exists(home) ? dev.yog.util.YogPackStager.mergeTags(Files.readString(home), json) : json);
                entries.clear();
            }
            // Nowhere to put them (no such tag here): they stay in the `yog:` tag.
            JsonArray values = new JsonArray();
            entries.forEach(values::add);
            for (String t : tags) {
                JsonObject ref = new JsonObject();
                ref.addProperty("id", "#" + t);
                ref.addProperty("required", false);
                values.add(ref);
            }
            // The game's content it holds whatever tags this host has.
            for (String b : BASE.getOrDefault(e.getKey(), List.of())) {
                JsonObject ref = new JsonObject();
                ref.addProperty("id", b);
                ref.addProperty("required", false);
                values.add(ref);
            }
            JsonObject out = new JsonObject();
            out.add("values", values);
            Files.createDirectories(file.getParent());
            Files.writeString(file, gson.toJson(out));
        }
    }

    /** {@code data/<ns>/tags/<registry>/<path>.json} in this version's layout. */
    private static Path tagFile(Path pack, String registry, String id, boolean mc121) {
        int colon = id.indexOf(':');
        String rel = "data/" + id.substring(0, colon) + "/tags/" + registry + "/" + id.substring(colon + 1) + ".json";
        return pack.resolve(YogDataLayout.remapDataPath(rel, mc121));
    }
}
