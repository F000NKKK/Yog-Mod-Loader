package dev.yog.script.yog;

import dev.yog.YogLog;
import dev.yog.api.YogBus;
import dev.yog.bus.YogEvents;
import dev.yog.bus.YogServices;
import java.util.function.BiConsumer;
import java.util.function.Consumer;

/**
 * Calls across sides as Yog Script writes them: a method's arguments a
 * record ({@code ShowChargeMessage}), sent on its channel to a player's
 * client or to the server, and run there when it arrives. A handler's
 * panic is logged, the game going on.
 */
public final class Messages {
    private Messages() {}

    /** {@code message} sent on {@code channel} to player {@code player}'s client. */
    public static void toClient(String player, String channel, Object message) {
        new YogServices.Net.SendToPlayer(player, channel, YogBus.bytes(message)).call();
    }

    /** {@code message} sent on {@code channel} to the server. */
    public static void toServer(String channel, Object message) {
        new YogServices.Net.SendToServer(channel, YogBus.bytes(message)).call();
    }

    /** What arrives on {@code channel} at a client (a {@code type}) given to {@code run}. */
    public static <M> void onClient(String channel, Class<M> type, Consumer<M> run) {
        YogEvents.Client.Net.Packet.on(channel, 0, p -> {
            handled(channel, () -> run.accept(YogBus.fromBytes(p.payload(), type)));
            return null;
        });
    }

    /** What arrives on {@code channel} at the server given to {@code run}, with who sent it. */
    public static <M> void onServer(String channel, Class<M> type, BiConsumer<Sender, M> run) {
        YogEvents.Server.Net.Packet.on(channel, 0, p -> {
            handled(channel, () -> run.accept(Sender.of(p.player()), YogBus.fromBytes(p.payload(), type)));
            return null;
        });
    }

    private static void handled(String channel, Runnable run) {
        try {
            run.run();
        } catch (RuntimeException e) {
            YogLog.error("Yog Script: a message on " + channel + " panicked: " + e.getMessage());
        }
    }
}
