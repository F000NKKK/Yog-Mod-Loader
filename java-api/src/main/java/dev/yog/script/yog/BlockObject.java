package dev.yog.script.yog;

import dev.yog.YogLog;
import dev.yog.bus.YogEvents;
import dev.yog.bus.YogTypes;
import dev.yog.script.Action1;
import dev.yog.script.Event;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;

/**
 * Yog Script's {@code Yog.BlockObject}: a block a mod declares, at
 * its place in a world. Made the first time something happens there (its
 * constructor subscribes), kept while the server runs, gone when it's
 * broken. A handler's panic is logged: the game goes on.
 */
public abstract class BlockObject {
    /** Where an object is: its world and its place in it. */
    private record At(String dimension, int x, int y, int z) {
        static At of(String dimension, YogTypes.BlockPos pos) {
            return new At(dimension, pos.x(), pos.y(), pos.z());
        }

        /** As its synced rows name their owner: the world, then the place. */
        String key() {
            return dimension + OWNER_SEP + x + OWNER_SEP + y + OWNER_SEP + z;
        }

        static At parse(String key) {
            String[] p = key.split(OWNER_SEP);
            return new At(p[0], Integer.parseInt(p[1]), Integer.parseInt(p[2]), Integer.parseInt(p[3]));
        }
    }

    /** What a block's synced rows say they're of, before its id. */
    private static final String SYNCED_KIND = dev.yog.YogNames.Kind.BLOCK + dev.yog.YogNames.Sep.NAMESPACE;
    /** Between a place's parts in its key (never in an id or a number). */
    private static final String OWNER_SEP = " ";

    /** Each declared block's maker, by its id. */
    private static final Map<String, Supplier<? extends BlockObject>> MAKERS = new ConcurrentHashMap<>();
    /** Whether its dispatcher listens already. */
    private static volatile boolean dispatching;
    /** The objects made so far, by where they are. */
    private static final Map<At, BlockObject> MADE = new ConcurrentHashMap<>();
    /** Where the object being made is: its constructor reads it. */
    private static final ThreadLocal<At> MAKING = new ThreadLocal<>();

    private final YogTypes.BlockPos pos;
    private final String dimension;

    public final Event<Action1<String>> Use = new Event<>();
    public final Event<Action1<String>> Place = new Event<>();
    public final Event<Action1<String>> Break = new Event<>();

    protected BlockObject() {
        At at = MAKING.get();
        this.pos = new YogTypes.BlockPos(at.x(), at.y(), at.z());
        this.dimension = at.dimension();
    }

    public YogTypes.BlockPos Pos() {
        return pos;
    }

    public String Dimension() {
        return dimension;
    }

    /** Block {@code id}'s objects made by {@code make}; the first one declared listens to the blocks' events. */
    public static void register(String id, Supplier<? extends BlockObject> make) {
        // Its dispatcher once, whichever generation declares first (a reload empties MAKERS of the old's).
        boolean first = !dispatching;
        dispatching = true;
        MAKERS.put(id, make);
        // A client's object at a place: what its synced rows go to.
        SyncedState.owners(SYNCED_KIND + id, owner -> found(id, At.parse(owner)) instanceof SyncedState.Synced s ? s : null);
        if (first) {
            listen();
        }
    }

    /** The shared dispatchers, once: the loader's own ({@link dev.yog.Generations#unowned}), not the mod's that first needs them. */
    private static void listen() {
        dev.yog.Generations.unowned(BlockObject::listenOnce);
    }

    private static void listenOnce() {
        YogEvents.Block.Use.on(0, e -> {
            raised(found(e.blockId(), At.of(e.dimension(), e.pos())), o -> o.Use, e.playerName());
            return null;
        });
        YogEvents.Block.Place.on(0, e -> {
            At at = At.of(e.dimension(), e.pos());
            MADE.remove(at);
            gone(e.blockId(), at);
            raised(found(e.blockId(), at), o -> o.Place, e.playerName());
            return null;
        });
        YogEvents.Block.Break.on(0, e -> {
            At at = At.of(e.dimension(), e.pos());
            raised(found(e.blockId(), at), o -> o.Break, e.playerName());
            if (MADE.remove(at) instanceof SyncedState.Synced s) {
                SyncedState.forget(s);
            }
            gone(e.blockId(), at);
            return null;
        });
    }

    /** The objects of classes {@code loaders} loaded (a generation a hot reload
     * replaces): no longer made; the new classes' objects are, as they're asked
     * for (taking what the old ones held, {@link Session}). */
    public static void retire(java.util.Set<ClassLoader> loaders) {
        MADE.values().removeIf(o -> loaders.contains(o.getClass().getClassLoader()));
        MAKERS.values().removeIf(m -> loaders.contains(m.getClass().getClassLoader()));
    }

    /** The object of block {@code id} at {@code at}: made now if it wasn't; none for a block no mod declared. */
    private static BlockObject found(String id, At at) {
        Supplier<? extends BlockObject> make = MAKERS.get(id);
        if (make == null) {
            return null;
        }
        return MADE.computeIfAbsent(at, p -> {
            MAKING.set(p);
            try {
                // Carried over by a hot reload: the same block's object of the old classes, its now.
                BlockObject o = Session.live(make.get(), Session.key(SYNCED_KIND + id, p.key()));
                // Its `saved` members: read in from its chunk's store.
                if (o instanceof State.Saved s) {
                    State.block(s, namespace(id), p.dimension(), p.x(), p.y(), p.z());
                }
                if (o instanceof SyncedState.Synced s) {
                    SyncedState.toAll(s, SYNCED_KIND + id, p.key());
                }
                return o;
            } finally {
                MAKING.remove();
            }
        });
    }

    /** A block of {@code id}'s mod gone from {@code at}: its records with it. */
    private static void gone(String id, At at) {
        if (MAKERS.containsKey(id)) {
            State.blockGone(namespace(id), at.dimension(), at.x(), at.y(), at.z());
        }
    }

    /** The mod an id is of: its namespace ({@code mymod:lamp}'s {@code mymod}). */
    private static String namespace(String id) {
        int sep = id.indexOf(dev.yog.YogNames.Sep.NAMESPACE);
        return sep < 0 ? id : id.substring(0, sep);
    }

    /** {@code o}'s event {@code which} raised with {@code player}; a panic logged, the game going on. */
    private static void raised(BlockObject o, java.util.function.Function<BlockObject, Event<Action1<String>>> which, String player) {
        if (o == null) {
            return;
        }
        try {
            List<Action1<String>> hs = which.apply(o).handlers();
            for (int i = 0, n = hs.size(); i < n; i++) hs.get(i).Invoke(player);
        } catch (RuntimeException e) {
            YogLog.error("Yog Script: a block's handler panicked: " + e.getMessage());
        }
    }
}
