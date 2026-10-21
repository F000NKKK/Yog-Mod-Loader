package dev.yog;

import net.minecraft.commands.arguments.ParticleArgument;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerLevel;
import com.mojang.brigadier.StringReader;

/** The host's particle actions: the methods {@code yog_event::particle} names. */
public final class YogParticleActions {
    private YogParticleActions() {}

    @YogServiceHost
    public static boolean spawnParticles(
            String dimension, double x, double y, double z,
            String particleTypeId, int count,
            double dx, double dy, double dz, double speed) {
        try {
            ServerLevel w = NativeBridge.worldFor(dimension);
            if (w == null) return false;
            if (particleTypeId.indexOf(' ') >= 0 || particleTypeId.indexOf('{') >= 0) {
                // Particle with options, /particle syntax (e.g. "minecraft:dust{color:[1,0,0],scale:1}")
                net.minecraft.core.particles.ParticleOptions parsed = ParticleArgument.readParticle(
                        new StringReader(particleTypeId), w.registryAccess());
                w.sendParticles(parsed, x, y, z, count, dx, dy, dz, speed);
                return true;
            }
            Identifier id = Identifier.tryParse(particleTypeId);
            if (id == null) return false;
            net.minecraft.core.particles.ParticleType<?> type = BuiltInRegistries.PARTICLE_TYPE.getValue(id);
            if (!(type instanceof net.minecraft.core.particles.ParticleOptions effect)) return false;
            w.sendParticles(effect, x, y, z, count, dx, dy, dz, speed);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
