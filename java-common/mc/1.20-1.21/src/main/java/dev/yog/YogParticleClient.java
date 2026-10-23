package dev.yog;

import java.util.function.BiConsumer;
import java.util.function.Function;
import net.minecraft.client.particle.ParticleProvider;
import net.minecraft.client.particle.SpriteSet;
import net.minecraft.world.phys.Vec3;

/** The client's side of the mods' particles: each type's provider, made from its pictures. */
public final class YogParticleClient {
    private YogParticleClient() {}

    /** Each mod particle's provider, to the loader's way of registering one with its pictures. */
    public static void providers(BiConsumer<YogParticles.Type, Function<SpriteSet, ParticleProvider<YogParticles.Options>>> out) {
        for (YogParticles.Type type : YogParticles.types()) {
            out.accept(type, sprites -> (o, level, x, y, z, dx, dy, dz) ->
                    new YogParticle(level, new Vec3(x, y, z), new Vec3(dx, dy, dz), o, sprites));
        }
    }
}
