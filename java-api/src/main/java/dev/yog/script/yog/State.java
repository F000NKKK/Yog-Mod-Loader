package dev.yog.script.yog;

import dev.yog.YogNames;
import dev.yog.bus.YogEvents;
import dev.yog.bus.YogServices;
import dev.yog.bus.YogTypes;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BiConsumer;
import java.util.function.Function;

/**
 * Yog Script's declared state ({@code saved}, docs/design/storage.md): the
 * live objects that keep some, each read in from its mod's store as it's
 * made and written out as the world saves. A record a saved member: its
 * key {@code Owner.Member}, its bytes the member's value (Yog Data).
 */
public final class State {
    private State() {}

    /** An object with saved members: what reads them in and writes them out. */
    public interface Saved {
        /** Each saved member read from its record ({@code null}: none yet). */
        void $load(Function<String, byte[]> read);

        /** Each saved member written to its record. */
        void $save(BiConsumer<String, byte[]> write);

        /** Its owner gone (a player left): it's no longer live. */
        void $forget();
    }

    /** Where an object's records are: its mod's store of a scope's instance. */
    private record Place(String mod, String scope, String instance) {
        YogTypes.Where where() {
            return new YogTypes.Where(scope, instance);
        }
    }

    private static final Map<Saved, Place> LIVE = new ConcurrentHashMap<>();
    private static volatile boolean listening;

    /** {@code made} (mod {@code mod}'s, of the game's store): read in, live. */
    public static <T extends Saved> T game(T made, String mod) {
        return live(made, new Place(mod, YogNames.Storage.GAME, ""));
    }

    /** {@code made} (mod {@code mod}'s, of the world's store): read in, live. */
    public static <T extends Saved> T world(T made, String mod) {
        return live(made, new Place(mod, YogNames.Storage.WORLD, ""));
    }

    /** {@code made} (of player {@code name}'s store, by its uuid): read in, live. */
    public static <T extends Saved> T player(T made, String mod, String name) {
        YogTypes.PlayerInfo info = new YogServices.Player.Info(name).call();
        String uuid = info == null ? name : info.uuid();
        return live(made, new Place(mod, YogNames.Storage.PLAYER, uuid));
    }

    private static <T extends Saved> T live(T made, Place at) {
        listen();
        made.$load(key -> {
            YogTypes.Record r = new YogServices.Storage.Read(at.mod(), at.where(), "", key).call();
            return r == null ? null : r.bytes();
        });
        LIVE.put(made, at);
        return made;
    }

    /** Every live object's saved members written to their records. */
    public static void saveAll() {
        LIVE.forEach(State::save);
    }

    private static void save(Saved o, Place at) {
        o.$save((key, bytes) -> new YogServices.Storage.Write(at.mod(), at.where(), "", key, bytes).call());
    }

    /** The world saving writes them; a player leaving writes and drops theirs. */
    private static void listen() {
        if (listening) {
            return;
        }
        synchronized (State.class) {
            if (listening) {
                return;
            }
            YogEvents.Server.Life.Saving.on(0, e -> {
                saveAll();
                return null;
            });
            YogEvents.Player.Session.Leave.on(0, e -> {
                LIVE.forEach((o, at) -> {
                    if (at.scope().equals(YogNames.Storage.PLAYER) && at.instance().equals(e.uuid())) {
                        save(o, at);
                        o.$forget();
                        LIVE.remove(o);
                    }
                });
                return null;
            });
            listening = true;
        }
    }
}
