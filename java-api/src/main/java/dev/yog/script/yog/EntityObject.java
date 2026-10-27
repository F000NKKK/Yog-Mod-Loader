package dev.yog.script.yog;

import dev.yog.YogLog;
import dev.yog.bus.YogEvents;
import dev.yog.script.Action;
import dev.yog.script.Action1;
import dev.yog.script.Event;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;
import java.util.function.Supplier;

/**
 * Yog Script's {@code Yog.EntityObject}: an entity a mod declares, one
 * object an entity of it — made the first time something happens to it
 * (its constructor subscribes), kept while the server runs, gone when it
 * dies (its {@code saved} and {@code synced} members, the entity's data,
 * with it). A handler's panic is
 * logged: the game goes on.
 */
public abstract class EntityObject {
    /** Each declared entity's maker, by its id. */
    private static final Map<String, Supplier<? extends EntityObject>> MAKERS = new ConcurrentHashMap<>();
    /** The objects made so far, by their entities' uuids. */
    private static final Map<String, EntityObject> MADE = new ConcurrentHashMap<>();
    /** The entity the object being made is: its constructor reads it. */
    private static final ThreadLocal<String[]> MAKING = new ThreadLocal<>();

    private final String uuid;
    private final String dimension;

    public final Event<Action> Spawn = new Event<>();
    public final Event<Action1<Float>> Damage = new Event<>();
    public final Event<Action> Death = new Event<>();

    protected EntityObject() {
        String[] at = MAKING.get();
        this.uuid = at[0];
        this.dimension = at[1];
    }

    public String Uuid() {
        return uuid;
    }

    public String Dimension() {
        return dimension;
    }

    /** Entity {@code id}'s objects made by {@code make}; the first one declared listens to entities' events. */
    public static void register(String id, Supplier<? extends EntityObject> make) {
        boolean first = MAKERS.isEmpty();
        MAKERS.put(id, make);
        if (first) {
            listen();
        }
    }

    private static void listen() {
        YogEvents.Entity.Life.Spawn.on(0, e -> {
            raised(found(e.entityType(), e.uuid(), e.dimension()), o -> o.Spawn.handlers().forEach(Action::Invoke));
            return null;
        });
        YogEvents.Entity.Combat.Damage.on(0, e -> {
            raised(found(e.entityType(), e.uuid(), e.dimension()), o -> o.Damage.handlers().forEach(h -> h.Invoke(e.amount())));
            return null;
        });
        YogEvents.Entity.Life.Death.on(0, e -> {
            raised(found(e.entityType(), e.uuid(), e.dimension()), o -> o.Death.handlers().forEach(Action::Invoke));
            EntityObject gone = MADE.remove(e.uuid());
            if (gone != null) {
                EntityState.gone(gone);
            }
            return null;
        });
    }

    /** The object of entity {@code uuid} (of {@code id}): made now if it wasn't; none for one no mod declared. */
    private static EntityObject found(String id, String uuid, String dimension) {
        Supplier<? extends EntityObject> make = MAKERS.get(id);
        if (make == null) {
            return null;
        }
        return MADE.computeIfAbsent(uuid, u -> {
            MAKING.set(new String[] {u, dimension});
            try {
                EntityObject o = make.get();
                // Its `saved` and `synced` members: the entity's data.
                EntityState.live(o, namespace(id), u);
                return o;
            } finally {
                MAKING.remove();
            }
        });
    }

    /**
     * Entity {@code uuid} as a {@code type}, on a client: made now, its
     * {@code saved} and {@code synced} members as the server sent them (to
     * the players who see it; their defaults for one it hasn't); none when
     * no mod declares {@code type}. Its {@code Dimension} is unknown here.
     */
    public static <T extends EntityObject> T Seen(Class<T> type, String uuid) {
        for (Map.Entry<String, Supplier<? extends EntityObject>> m : MAKERS.entrySet()) {
            MAKING.set(new String[] {uuid, ""});
            EntityObject o;
            try {
                o = m.getValue().get();
            } finally {
                MAKING.remove();
            }
            if (type.isInstance(o)) {
                EntityState.seen(o, namespace(m.getKey()), uuid);
                return type.cast(o);
            }
        }
        return null;
    }

    /** The mod an id is of: its namespace ({@code mymod:golem}'s {@code mymod}). */
    private static String namespace(String id) {
        int sep = id.indexOf(dev.yog.YogNames.Sep.NAMESPACE);
        return sep < 0 ? id : id.substring(0, sep);
    }

    /** {@code raise} on {@code o}; a panic logged, the game going on. */
    private static void raised(EntityObject o, Consumer<EntityObject> raise) {
        if (o == null) {
            return;
        }
        try {
            raise.accept(o);
        } catch (RuntimeException e) {
            YogLog.error("Yog Script: an entity's handler panicked: " + e.getMessage());
        }
    }
}
