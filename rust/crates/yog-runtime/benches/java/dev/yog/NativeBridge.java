package dev.yog;

/** Stand-in for the host's NativeBridge: the same shapes, no game behind them. */
public final class NativeBridge {
    private NativeBridge() {}

    public static String getBlock(String dimension, int x, int y, int z) {
        return "minecraft:stone";
    }

    public static boolean teleport(String player, double x, double y, double z) {
        return true;
    }

    public static boolean sendToPlayer(String player, String channel, byte[] payload) {
        return payload.length > 0;
    }
}
