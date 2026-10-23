package dev.yog.content;

import com.mojang.serialization.Codec;
import com.mojang.serialization.MapCodec;
import dev.yog.bus.YogTypes;
import dev.yog.data.YogContent;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.particles.DustParticleOptions;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.particles.ParticleType;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.resources.ResourceLocation;
import org.joml.Vector3f;

/**
 * The mods' particles ({@code .yparticle}): each a particle type of the
 * game's, its options a colour; drawn by {@link YogParticle} as its look says.
 * And what a spawn makes of the options a mod gives ({@link #options}).
 */
public final class YogParticles {
    private YogParticles() {}

    /** A spawn's colour when it gives none, and the game's dust's size. */
    private static final int WHITE = 0xFFFFFF;
    private static final float DUST_SCALE = 1f;
    /** The options' field holding the colour. */
    private static final String COLOR = "color";

    /** A mod's particle type: its look, and its options' codecs. */
    public static final class Type extends ParticleType<Options> {
        final YogTypes.Particle look;
        private final MapCodec<Options> codec =
                Codec.INT.optionalFieldOf(COLOR, WHITE).xmap(c -> new Options(this, c), Options::color);
        private final StreamCodec<RegistryFriendlyByteBuf, Options> stream =
                ByteBufCodecs.INT.map(c -> new Options(this, c), Options::color).cast();

        Type(YogTypes.Particle look) {
            super(false);
            this.look = look;
        }

        @Override
        public MapCodec<Options> codec() {
            return codec;
        }

        @Override
        public StreamCodec<? super RegistryFriendlyByteBuf, Options> streamCodec() {
            return stream;
        }
    }

    /** One spawn of a mod's particle: its colour ({@code 0xRRGGBB}). */
    public record Options(Type type, int color) implements ParticleOptions {
        @Override
        public ParticleType<?> getType() {
            return type;
        }
    }

    /** A colour channel's largest value. */
    private static final float CHANNEL = 255f;

    /** {@code 0xRRGGBB} as the game's dust takes it. */
    private static Vector3f rgb(int c) {
        return new Vector3f((c >> 16 & 0xFF) / CHANNEL, (c >> 8 & 0xFF) / CHANNEL, (c & 0xFF) / CHANNEL);
    }

    /** The mods' particle types, by id. */
    private static final Map<String, Type> TYPES = new LinkedHashMap<>();

    /** The mods' particles, with {@link YogRegistrar}. */
    static void schedule(YogContent content) {
        YogRegistrar.add(Registries.PARTICLE_TYPE, sink -> {
            for (YogTypes.ParticleDef p : content.particles) {
                Type type = new Type(p.look());
                TYPES.put(p.id(), type);
                sink.register(Registries.PARTICLE_TYPE, ResourceLocation.parse(p.id()), type);
            }
        });
    }

    /** The mods' particle types (the client gives each its provider, {@link YogParticleClient}). */
    public static Iterable<Type> types() {
        return TYPES.values();
    }

    /**
     * What particle {@code id} with a mod's options is on this version: a
     * mod's in {@code color}, the game's dust in {@code color} at
     * {@code scale}, another of the game's as it is; null for none.
     */
    public static ParticleOptions options(String id, Long color, Float scale) {
        int c = color == null ? WHITE : color.intValue();
        Type mine = TYPES.get(id);
        if (mine != null) return new Options(mine, c);
        ResourceLocation key = ResourceLocation.tryParse(id);
        ParticleType<?> type = key == null ? null : BuiltInRegistries.PARTICLE_TYPE.get(key);
        if (type == ParticleTypes.DUST) return new DustParticleOptions(rgb(c), scale == null ? DUST_SCALE : scale);
        return type instanceof ParticleOptions plain ? plain : null;
    }
}
