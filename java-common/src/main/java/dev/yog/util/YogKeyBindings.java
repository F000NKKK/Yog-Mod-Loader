package dev.yog.util;

import dev.yog.bus.YogEvents;
import dev.yog.bus.YogServices;
import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.BooleanSupplier;

/**
 * The mods' key bindings, the part every host shares: what the
 * runtime declares, which game key each became, the presses since the last
 * tick, and the names of the mods' categories in the controls. A host only
 * makes the game's key (its version's class) and registers it.
 */
public final class YogKeyBindings {
    private YogKeyBindings() {}

    /** A mod's key binding: its id, its label's translation key, its default GLFW key, its mod's name. */
    public record Binding(String id, String name, int defaultKey, String modName) {
        /** The mod's namespace: the id's. */
        public String namespace() {
            int colon = id.indexOf(':');
            return colon > 0 ? id.substring(0, colon) : id;
        }

        /** The translation key of the mod's category in the controls. */
        public String categoryKey() {
            return YogKeyBindings.categoryKey(namespace());
        }
    }

    /**
     * A mod's category's path: 26.x names a category by an id
     * ({@code <ns>:keys}, translated as {@code key.category.<ns>.keys}),
     * older versions by that translation key itself — the same key either way.
     */
    public static final String CATEGORY_PATH = "keys";
    private static final String CATEGORY_PREFIX = "key.category.";

    private static final List<Binding> DECLARED = new ArrayList<>();
    private static final List<Map.Entry<String, BooleanSupplier>> KEYS = new ArrayList<>();

    /** The translation key of {@code namespace}'s category: {@code key.category.<ns>.keys}. */
    public static String categoryKey(String namespace) {
        return CATEGORY_PREFIX + namespace + "." + CATEGORY_PATH;
    }

    /**
     * The bindings the mods declare (the {@code key_binding/declared} service),
     * asked once: the pack's category names may want them before the loader
     * registers the keys (Forge 1.19.2 stages its packs first).
     */
    public static List<Binding> declared() {
        if (DECLARED.isEmpty()) {
            for (YogTypes.CategorizedBinding b : new YogServices.KeyBinding.Declared().call()) {
                DECLARED.add(new Binding(b.id(), b.name(), b.defaultKey(), b.category()));
            }
        }
        return List.copyOf(DECLARED);
    }

    /** What the game's own bindings' names start with: {@code key.sprint}. */
    private static final String GAME_BINDING_PREFIX = "key.";

    /**
     * The game's name of a binding: a mod's by its id ({@code mymod:xray},
     * the name it declared), the game's by its own ({@code sprint} →
     * {@code key.sprint}).
     */
    public static String nameOf(String binding) {
        if (!binding.contains(dev.yog.YogNames.Sep.NAMESPACE)) return GAME_BINDING_PREFIX + binding;
        for (Binding b : DECLARED) {
            if (b.id().equals(binding)) return b.name();
        }
        return binding;
    }

    /** The game's key for binding {@code id}: {@code clicked} answers one press since the last (and takes it). */
    public static void track(String id, BooleanSupplier clicked) {
        KEYS.add(Map.entry(id, clicked));
    }

    /** Every press since the last tick, to the mods (each binding's own event). */
    public static void tick() {
        for (Map.Entry<String, BooleanSupplier> k : KEYS) {
            while (k.getValue().getAsBoolean()) new YogEvents.Client.Input.Binding(k.getKey()).emit();
        }
    }

    /** Each mod's category's name in the controls: its translation key → the mod's name. */
    public static Map<String, String> categoryNames() {
        Map<String, String> out = new LinkedHashMap<>();
        for (Binding b : declared()) out.putIfAbsent(b.categoryKey(), b.modName());
        return out;
    }
}
