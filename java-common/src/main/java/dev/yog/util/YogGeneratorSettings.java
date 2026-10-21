package dev.yog.util;

import com.mojang.serialization.Codec;
import com.mojang.serialization.MapCodec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import dev.yog.YogNames.CallbackGenerator;

/**
 * What a dimension's {@code yog:callback_generator} says besides its biomes:
 * the generator in code its chunks come from, its height, and whether it
 * has structures, features and spawned mobs (each by default).
 */
public record YogGeneratorSettings(String generatorTypeId, int minY, int height, boolean structures, boolean features, boolean mobSpawning) {
    /** Its fields in the generator's JSON, beside the biome source. */
    public static final MapCodec<YogGeneratorSettings> MAP_CODEC = RecordCodecBuilder.mapCodec(inst -> inst.group(
                    Codec.STRING.fieldOf(CallbackGenerator.GENERATOR_TYPE_ID).forGetter(YogGeneratorSettings::generatorTypeId),
                    Codec.INT.fieldOf(CallbackGenerator.MIN_Y).forGetter(YogGeneratorSettings::minY),
                    Codec.INT.fieldOf(CallbackGenerator.HEIGHT).forGetter(YogGeneratorSettings::height),
                    Codec.BOOL.optionalFieldOf(CallbackGenerator.STRUCTURES, true).forGetter(YogGeneratorSettings::structures),
                    Codec.BOOL.optionalFieldOf(CallbackGenerator.FEATURES, true).forGetter(YogGeneratorSettings::features),
                    Codec.BOOL.optionalFieldOf(CallbackGenerator.MOB_SPAWNING, true).forGetter(YogGeneratorSettings::mobSpawning))
            .apply(inst, YogGeneratorSettings::new));
}
