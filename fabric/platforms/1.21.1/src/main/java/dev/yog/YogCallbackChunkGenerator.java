package dev.yog;

import com.mojang.serialization.Codec;
import com.mojang.serialization.MapCodec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import net.minecraft.block.BlockState;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.ChunkRegion;
import net.minecraft.world.Heightmap;
import net.minecraft.world.HeightLimitView;
import net.minecraft.world.biome.source.BiomeAccess;
import net.minecraft.world.biome.source.BiomeSource;
import net.minecraft.world.chunk.Chunk;
import net.minecraft.world.gen.GenerationStep;
import net.minecraft.world.gen.StructureAccessor;
import net.minecraft.world.gen.chunk.Blender;
import net.minecraft.world.gen.chunk.ChunkGenerator;
import net.minecraft.world.gen.chunk.VerticalBlockSample;
import net.minecraft.world.gen.noise.NoiseConfig;

/**
 * Bridges vanilla's {@link ChunkGenerator} to a mod-registered Rust closure
 * (see {@code Registry::register_chunk_generator} in the {@code yog-dimensions}
 * crate). Registered under {@code yog:callback_generator} in
 * {@code Registries.CHUNK_GENERATOR}; a dimension's JSON references it via
 * {@code "type": "yog:callback_generator", "generator_type_id": "..."}.
 *
 * <p>{@link #populateNoise} makes ONE native call per chunk
 * ({@link NativeBridge#nativeGenerateChunk}); every block the mod's closure
 * places arrives via a nested call back into
 * {@link #setBlockInGeneratingChunk} on the same worker thread — see the
 * {@code ChunkWriter}/{@code YogChunkWriterApi} docs on the Rust side.
 *
 * <p>This is a minimal generator: surface decoration, carvers, and mob
 * spawning are no-ops — the mod's closure is responsible for all terrain via
 * {@code ChunkWriter::set_block}.
 */
public class YogCallbackChunkGenerator extends ChunkGenerator {
    public static final MapCodec<YogCallbackChunkGenerator> CODEC = RecordCodecBuilder.mapCodec(inst -> inst.group(
                    BiomeSource.CODEC.fieldOf("biome_source").forGetter(g -> g.biomeSource),
                    Codec.STRING.fieldOf("generator_type_id").forGetter(g -> g.generatorTypeId),
                    Codec.INT.fieldOf("min_y").forGetter(g -> g.minY),
                    Codec.INT.fieldOf("height").forGetter(g -> g.height))
            .apply(inst, YogCallbackChunkGenerator::new));

    /** The chunk currently being filled on this worker thread, if any. */
    private static final ThreadLocal<Chunk> CURRENT_CHUNK = new ThreadLocal<>();

    private final String generatorTypeId;
    private final int minY;
    private final int height;

    public YogCallbackChunkGenerator(BiomeSource biomeSource, String generatorTypeId, int minY, int height) {
        super(biomeSource);
        this.generatorTypeId = generatorTypeId;
        this.minY = minY;
        this.height = height;
    }

    /** Register {@link #CODEC} under {@code yog:callback_generator}. Call once at mod-init. */
    public static void registerCodec() {
        Registry.register(Registries.CHUNK_GENERATOR, Identifier.of("yog", "callback_generator"), CODEC);
    }

    @Override
    protected MapCodec<? extends ChunkGenerator> getCodec() {
        return CODEC;
    }

    @Override
    public CompletableFuture<Chunk> populateNoise(
            Blender blender, NoiseConfig noiseConfig, StructureAccessor structureAccessor, Chunk chunk) {
        net.minecraft.util.math.ChunkPos pos = chunk.getPos();
        CURRENT_CHUNK.set(chunk);
        try {
            NativeBridge.nativeGenerateChunk(generatorTypeId, pos.x, pos.z, minY, height);
        } finally {
            CURRENT_CHUNK.remove();
        }
        return CompletableFuture.completedFuture(chunk);
    }

    /**
     * Called by native code (via JNI, on the same thread that's inside
     * {@link #populateNoise}) once per block the mod's Rust closure places.
     * `x`/`z` are chunk-local (0..16); `y` is world-absolute.
     */
    public static boolean setBlockInGeneratingChunk(int x, int y, int z, String blockId) {
        Chunk chunk = CURRENT_CHUNK.get();
        if (chunk == null) {
            return false;
        }
        Identifier ident = Identifier.tryParse(blockId);
        if (ident == null || !Registries.BLOCK.containsId(ident)) {
            return false;
        }
        chunk.setBlockState(new BlockPos(x, y, z), Registries.BLOCK.get(ident).getDefaultState(), false);
        return true;
    }

    // ── Everything below is intentionally minimal — terrain shaping is the
    // registered closure's job via ChunkWriter::set_block, not this class's. ──

    @Override
    public void buildSurface(ChunkRegion region, StructureAccessor structures, NoiseConfig noiseConfig, Chunk chunk) {
    }

    @Override
    public void carve(
            ChunkRegion chunkRegion, long seed, NoiseConfig noiseConfig, BiomeAccess biomeAccess,
            StructureAccessor structureAccessor, Chunk chunk, GenerationStep.Carver carverStep) {
    }

    @Override
    public void populateEntities(ChunkRegion region) {
    }

    @Override
    public int getWorldHeight() {
        return height;
    }

    @Override
    public int getSeaLevel() {
        return minY + height / 3;
    }

    @Override
    public int getMinimumY() {
        return minY;
    }

    @Override
    public int getHeight(int x, int z, Heightmap.Type heightmap, HeightLimitView world, NoiseConfig noiseConfig) {
        return minY;
    }

    @Override
    public VerticalBlockSample getColumnSample(int x, int z, HeightLimitView world, NoiseConfig noiseConfig) {
        return new VerticalBlockSample(minY, new BlockState[0]);
    }

    @Override
    public void getDebugHudText(List<String> text, NoiseConfig noiseConfig, BlockPos pos) {
    }
}
