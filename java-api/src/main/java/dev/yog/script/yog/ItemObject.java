package dev.yog.script.yog;

import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.bus.YogServices;
import dev.yog.bus.YogTypes;
import dev.yog.bus.YogEvents;
import dev.yog.script.Action1;
import dev.yog.script.Event;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;

/**
 * Yog Script's {@code Yog.ItemObject}: an item a mod declares, as a
 * stack of it is used — made for each use (its constructor subscribes)
 * until stacks keep state. A handler's panic is logged: the game goes on.
 */
public abstract class ItemObject {
    /** Each declared item's maker, by its id. */
    private static final Map<String, Supplier<? extends ItemObject>> MAKERS = new ConcurrentHashMap<>();

    public final Event<Action1<String>> Use = new Event<>();

    /** Item {@code id}'s objects made by {@code make}; the first one declared listens to items' uses. */
    public static void register(String id, Supplier<? extends ItemObject> make) {
        boolean first = MAKERS.isEmpty();
        MAKERS.put(id, make);
        if (first) {
            // The loader's own dispatcher, not the first mod's (dev.yog.Generations#unowned).
            dev.yog.Generations.unowned(() -> YogEvents.Player.Action.UseItem.on(0, e -> {
                Supplier<? extends ItemObject> m = MAKERS.get(e.itemId());
                if (m != null) {
                    try {
                        ItemObject o = m.get();
                        String player = e.playerName();
                        // Its `saved` members: the stack's, read before and written after.
                        String mod = namespace(e.itemId());
                        if (o instanceof State.Saved s) {
                            State.heldLoad(s, mod, player, e.offHand());
                        }
                        List<Action1<String>> hs = o.Use.handlers();
                        for (int i = 0, n = hs.size(); i < n; i++) hs.get(i).Invoke(player);
                        if (o instanceof State.Saved s) {
                            State.heldSave(s, mod, player, e.offHand());
                        }
                    } catch (RuntimeException ex) {
                        YogLog.error("Yog Script: an item's handler panicked: " + ex.getMessage());
                    }
                }
                return null;
            }));
        }
    }

    /**
     * The stack this client's player holds ({@code offHand}: in its other
     * hand) as a {@code type}, its {@code saved} and {@code synced} members
     * read from the stack's data (the server's, as the game sent it); none
     * when it holds no {@code type}.
     */
    public static <T extends ItemObject> T Held(Class<T> type, boolean offHand) {
        YogTypes.Stack held = new YogServices.Client.HeldItem(offHand).call();
        Supplier<? extends ItemObject> make = held == null ? null : MAKERS.get(held.item());
        ItemObject o = make == null ? null : make.get();
        if (!type.isInstance(o)) {
            return null;
        }
        if (o instanceof State.Saved s) {
            String mod = namespace(held.item());
            s.$load(key -> {
                YogTypes.Record r = new YogServices.Client.HeldItemData(offHand, State.modKey(mod, key)).call();
                return r == null ? null : r.bytes();
            });
        }
        return type.cast(o);
    }

    /** The mod an id is of: its namespace ({@code mymod:wand}'s {@code mymod}). */
    private static String namespace(String id) {
        int sep = id.indexOf(YogNames.Sep.NAMESPACE);
        return sep < 0 ? id : id.substring(0, sep);
    }
}
