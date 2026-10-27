package dev.yog.script.yog;

import dev.yog.bus.YogEvents;
import dev.yog.bus.YogServices;
import dev.yog.bus.YogTypes;
import java.util.Arrays;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * An entity object's declared state (docs/design/storage.md): its
 * {@code saved} and {@code synced} members kept as the runtime's entity
 * data, each under {@code mod:Owner.Member} — saved with the entity (its
 * {@code saved} ones), sent to the players who see it, gone with it.
 * Written as it changes, once a tick.
 */
final class EntityState {
    private EntityState() {}

    /** A live object's entity, its mod, and what each key was last written as. */
    private record Live(String mod, String uuid, Map<String, byte[]> written) {}

    private static final Map<EntityObject, Live> LIVE = new ConcurrentHashMap<>();
    private static volatile boolean listening;

    /** {@code o} (entity {@code uuid}'s, of mod {@code mod}): its saved members read in, live. */
    static void live(EntityObject o, String mod, String uuid) {
        if (!(o instanceof State.Saved) && !(o instanceof SyncedState.Synced)) {
            return;
        }
        if (o instanceof State.Saved s) {
            s.$load(key -> bytes(new YogServices.Entity.Data(uuid, State.modKey(mod, key)).call()));
        }
        listen();
        LIVE.put(o, new Live(mod, uuid, new ConcurrentHashMap<>()));
    }

    /** {@code o}'s entity gone: no longer live (the runtime drops its data). */
    static void gone(EntityObject o) {
        LIVE.remove(o);
    }

    /** {@code o} (made on a client for entity {@code uuid}, of mod {@code mod}): its members as this client was sent them. */
    static void seen(EntityObject o, String mod, String uuid) {
        if (o instanceof State.Saved s) {
            s.$load(key -> bytes(new YogServices.Client.EntityData(uuid, State.modKey(mod, key)).call()));
        }
        if (o instanceof SyncedState.Synced s) {
            s.$sync((key, current) -> {
                byte[] sent = bytes(new YogServices.Client.EntityData(uuid, State.modKey(mod, key)).call());
                if (sent != null) {
                    s.$synced(key, sent);
                }
            });
        }
    }

    private static byte[] bytes(YogTypes.DataValue v) {
        return v == null ? null : v.bytes();
    }

    /** Each live object's changed members written: its saved ones kept with the entity. */
    private static void tick() {
        LIVE.forEach((o, at) -> {
            if (o instanceof State.Saved s) {
                s.$save((key, bytes) -> write(at, key, bytes, true));
            }
            if (o instanceof SyncedState.Synced s) {
                s.$sync((key, bytes) -> write(at, key, bytes, false));
            }
        });
    }

    /** {@code key} set to {@code bytes} unless that's what it was last set to (a saved and synced member's once). */
    private static void write(Live at, String key, byte[] bytes, boolean persist) {
        if (Arrays.equals(at.written().get(key), bytes)) {
            return;
        }
        at.written().put(key, bytes);
        new YogServices.Entity.SetData(at.uuid(), State.modKey(at.mod(), key), new YogTypes.DataValue(bytes), persist).call();
    }

    private static void listen() {
        if (listening) {
            return;
        }
        synchronized (EntityState.class) {
            if (listening) {
                return;
            }
            YogEvents.Server.Game.Tick.on(0, e -> {
                tick();
                return null;
            });
            listening = true;
        }
    }
}
