package dev.yog;

import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.server.level.ServerLevel;

/** The host's particle actions: the methods {@code yog_event::particle} names. */
public final class YogParticleActions {
    private YogParticleActions() {}

    /** {@code count} of particle {@code id} (a mod's or the game's, with its options) around a point. */
    @YogServiceHost
    public static boolean spawnParticles(
            String dimension, double x, double y, double z,
            String id, Long color, Float scale, int count,
            double dx, double dy, double dz, double speed) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        ParticleOptions options = YogParticles.options(id, color, scale);
        if (w == null || options == null) return false;
        w.sendParticles(options, x, y, z, count, dx, dy, dz, speed);
        return true;
    }
}
