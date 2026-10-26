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
 * Yog Script's {@code System.Yog.ItemObject}: an item a mod declares, as a
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
                        List<Action1<String>> hs = m.get().Use.handlers();
                        String player = e.playerName();
                        for (int i = 0, n = hs.size(); i < n; i++) hs.get(i).Invoke(player);
                    } catch (RuntimeException ex) {
                        YogLog.error("Yog Script: an item's handler panicked: " + ex.getMessage());
                    }
                }
                return null;
            });
        }
    }
}
