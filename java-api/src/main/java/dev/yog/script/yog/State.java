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

    /** Where an object's records are: its mod's store of a scope's
     * instance, under its owner there (a block's place in its chunk; empty
     * when the store is its own). */
    private record Place(String mod, String scope, String instance, String owner) {
        YogTypes.Where where() {
            return new YogTypes.Where(scope, instance);
        }
    }

    private static final Map<Saved, Place> LIVE = new ConcurrentHashMap<>();
    private static volatile boolean listening;

    /** {@code made} (mod {@code mod}'s, of the game's store): read in, live. */
    public static <T extends Saved> T game(T made, String mod) {
        return live(made, new Place(mod, YogNames.Storage.GAME, "", ""));
    }

    /** {@code made} (mod {@code mod}'s, of the world's store): read in, live. */
    public static <T extends Saved> T world(T made, String mod) {
        return live(made, new Place(mod, YogNames.Storage.WORLD, "", ""));
    }

    /** {@code made} (of player {@code name}'s store, by its uuid): read in, live. */
    public static <T extends Saved> T player(T made, String mod, String name) {
        YogTypes.PlayerInfo info = new YogServices.Player.Info(name).call();
        String uuid = info == null ? name : info.uuid();
        return live(made, new Place(mod, YogNames.Storage.PLAYER, uuid, ""));
    }

    /** {@code made} (an item's object, as {@code player} uses the stack
     * in a hand): its {@code saved} members read from the stack's data,
     * each under {@code mod:Owner.Member}. A stack's state travels with it:
     * no store, and nothing live. */
    public static void heldLoad(Saved made, String mod, String player, boolean offHand) {
        made.$load(key -> {
            YogTypes.Record r = new YogServices.Player.HeldItemData(player, offHand, stackKey(mod, key)).call();
            return r == null ? null : r.bytes();
        });
    }

    /** {@code made}'s {@code saved} members written to the stack's data, as it's changed. */
    public static void heldSave(Saved made, String mod, String player, boolean offHand) {
        made.$save((key, bytes) -> new YogServices.Player.SetHeldItemData(player, offHand, stackKey(mod, key), bytes).call());
    }

    /** A record's key in a stack's data: its mod's, then its own. */
    private static String stackKey(String mod, String key) {
        return mod + YogNames.Sep.NAMESPACE + key;
    }

    /** {@code made} (entity {@code uuid}'s object): read in from its own store, live. */
    public static <T extends Saved> T entity(T made, String mod, String uuid) {
        return live(made, new Place(mod, YogNames.Storage.ENTITY, uuid, ""));
    }

    /** Entity {@code uuid} dead for good: its records removed, its object no longer live. */
    public static void entityGone(String mod, String uuid) {
        Place at = new Place(mod, YogNames.Storage.ENTITY, uuid, "");
        LIVE.entrySet().removeIf(e -> e.getValue().equals(at));
        new YogServices.Storage.Remove(at.mod(), at.where(), at.owner(), null).call();
    }

    /** {@code made} (a block's object at {@code x y z} in {@code dimension}):
     * read in from its chunk's store, under its place there; live. */
    public static <T extends Saved> T block(T made, String mod, String dimension, int x, int y, int z) {
        return live(made, chunkAt(mod, dimension, x, y, z));
    }

    /** The block at {@code x y z} gone (broken, or another placed there): its
     * records removed, its object (if made) no longer live. */
    public static void blockGone(String mod, String dimension, int x, int y, int z) {
        Place at = chunkAt(mod, dimension, x, y, z);
        LIVE.entrySet().removeIf(e -> e.getValue().equals(at));
        new YogServices.Storage.Remove(at.mod(), at.where(), at.owner(), null).call();
    }

    /** Block {@code x y z}'s records' place: its chunk's store, its place in it. */
    private static Place chunkAt(String mod, String dimension, int x, int y, int z) {
        String sep = YogNames.Storage.SEP;
        String chunk = dimension + sep + (x >> CHUNK_BITS) + sep + (z >> CHUNK_BITS);
        String owner = (x & CHUNK_MASK) + sep + y + sep + (z & CHUNK_MASK);
        return new Place(mod, YogNames.Storage.CHUNK, chunk, owner);
    }

    /** A chunk's width, as bits of a block's coordinate (16 blocks), and the mask of a block's place in it. */
    private static final int CHUNK_BITS = YogNames.Units.CHUNK_BITS;
    private static final int CHUNK_MASK = (1 << CHUNK_BITS) - 1;

    private static <T extends Saved> T live(T made, Place at) {
        listen();
        made.$load(key -> {
            YogTypes.Record r = new YogServices.Storage.Read(at.mod(), at.where(), at.owner(), key).call();
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
        o.$save((key, bytes) -> new YogServices.Storage.Write(at.mod(), at.where(), at.owner(), key, bytes).call());
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
