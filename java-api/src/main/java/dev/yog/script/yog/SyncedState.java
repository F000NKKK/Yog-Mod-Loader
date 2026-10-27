package dev.yog.script.yog;

import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.api.YogBus;
import dev.yog.bus.YogEvents;
import dev.yog.bus.YogServices;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BiConsumer;
import java.util.function.Function;

/**
 * Yog Script's {@code synced} members (docs/design/storage.md §4): the
 * server's live objects keeping some, each tick their members encoded and
 * the changed ones sent to the clients that see their owner (a row each,
 * on {@code yog:synced}); a client's object of the same owner gets the
 * row's value, its reads plain field reads. A player joining gets every
 * row there is.
 */
public final class SyncedState {
    private SyncedState() {}

    /** An object with synced members: what writes them out and takes one in. */
    public interface Synced {
        /** Each synced member encoded, by its key ({@code Owner.Member}). */
        void $sync(BiConsumer<String, byte[]> out);

        /** Synced member {@code key} given its value's bytes (on a client). */
        void $synced(String key, byte[] bytes);

        /** Its owner gone (a player left): it's no longer live. */
        void $forget();
    }

    /** A synced member's value, as it goes to a client: its owner and its bytes. */
    public record Row(String kind, String owner, String key, byte[] bytes) {}

    /** What an object's rows say of its owner, and who gets them ({@code ""}: everyone). */
    private record Owner(String kind, String owner, String to) {}

    /** What a kind's owner names on a client: its object. */
    private static final Map<String, Function<String, Synced>> OWNERS = new ConcurrentHashMap<>();
    private static final Map<Synced, Owner> LIVE = new ConcurrentHashMap<>();
    /** What each object's members were last sent as, by key. */
    private static final Map<Synced, Map<String, byte[]>> SENT = new ConcurrentHashMap<>();
    private static volatile boolean listening;

    /** Kind {@code kind}'s object of an owner, on a client: {@code made}. */
    public static void owners(String kind, Function<String, Synced> made) {
        OWNERS.put(kind, made);
        listen();
    }

    /** {@code o} (of kind {@code kind}, owner {@code owner}) synced to player {@code to} only. */
    public static <T extends Synced> T toPlayer(T o, String kind, String owner) {
        return live(o, new Owner(kind, owner, owner));
    }

    /** {@code o} (of kind {@code kind}, owner {@code owner}) synced to every player. */
    public static <T extends Synced> T toAll(T o, String kind, String owner) {
        return live(o, new Owner(kind, owner, ""));
    }

    /** {@code o} no longer live (its owner gone). */
    public static void forget(Synced o) {
        LIVE.remove(o);
        SENT.remove(o);
    }

    private static <T extends Synced> T live(T o, Owner at) {
        listen();
        LIVE.put(o, at);
        return o;
    }

    /** Each live object's changed members sent to who sees them. */
    private static void tick() {
        for (Map.Entry<Synced, Owner> e : LIVE.entrySet()) {
            Owner at = e.getValue();
            Map<String, byte[]> sent = SENT.computeIfAbsent(e.getKey(), k -> new ConcurrentHashMap<>());
            List<Row> changed = new ArrayList<>();
            e.getKey().$sync((key, bytes) -> {
                if (!Arrays.equals(sent.get(key), bytes)) {
                    sent.put(key, bytes);
                    changed.add(new Row(at.kind(), at.owner(), key, bytes));
                }
            });
            if (!changed.isEmpty()) {
                List<String> players = audience(at);
                changed.forEach(row -> send(players, row));
            }
        }
    }

    /** Who gets {@code at}'s rows: its one player, or every one online. */
    private static List<String> audience(Owner at) {
        return at.to().isEmpty() ? new YogServices.Player.Online().call() : List.of(at.to());
    }

    private static void send(List<String> players, Row row) {
        byte[] payload = YogBus.bytes(row);
        for (String p : players) {
            new YogServices.Net.SendToPlayer(p, YogNames.Yog.SYNCED, payload).call();
        }
    }

    /** Player {@code name} joined: every row of what they see. */
    private static void joined(String name) {
        LIVE.forEach((o, at) -> {
            if (at.to().isEmpty() || at.to().equals(name)) {
                o.$sync((key, bytes) -> send(List.of(name), new Row(at.kind(), at.owner(), key, bytes)));
            }
        });
    }

    /** A row arrived at a client: its owner's object takes it. */
    private static void received(Row row) {
        Function<String, Synced> made = OWNERS.get(row.kind());
        if (made == null) {
            return;
        }
        Synced o = made.apply(row.owner());
        if (o != null) {
            o.$synced(row.key(), row.bytes());
        }
    }

    private static void listen() {
        if (listening) {
            return;
        }
        synchronized (SyncedState.class) {
            if (listening) {
                return;
            }
            YogEvents.Server.Game.Tick.on(0, e -> {
                tick();
                return null;
            });
            YogEvents.Player.Session.Join.on(0, e -> {
                joined(e.playerName());
                return null;
            });
            YogEvents.Player.Session.Leave.on(0, e -> {
                LIVE.forEach((o, at) -> {
                    if (at.to().equals(e.playerName())) {
                        o.$forget();
                        forget(o);
                    }
                });
                return null;
            });
            YogEvents.Client.Net.Packet.on(YogNames.Yog.SYNCED, 0, p -> {
                try {
                    received(YogBus.fromBytes(p.payload(), Row.class));
                } catch (RuntimeException ex) {
                    YogLog.error("Yog Script: a synced value didn't arrive: " + ex.getMessage());
                }
                return null;
            });
            listening = true;
        }
    }
}
