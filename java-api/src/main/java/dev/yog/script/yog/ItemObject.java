package dev.yog.script.yog;

import dev.yog.YogLog;
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
            YogEvents.Player.Action.UseItem.on(0, e -> {
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
            });
        }
    }

    /** The mod an id is of: its namespace ({@code mymod:wand}'s {@code mymod}). */
    private static String namespace(String id) {
        int sep = id.indexOf(dev.yog.YogNames.Sep.NAMESPACE);
        return sep < 0 ? id : id.substring(0, sep);
    }
}
