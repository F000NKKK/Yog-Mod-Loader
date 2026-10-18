package dev.yog;

import com.mojang.serialization.Codec;
import com.mojang.serialization.MapCodec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.WorldGenRegion;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.NoiseColumn;
import net.minecraft.world.level.StructureManager;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.BiomeSource;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;

/**
 * Bridges vanilla's {@link ChunkGenerator} to a mod-registered Rust closure
 * (see {@code Registry::register_chunk_generator} in the {@code yog-dimensions}
 * crate). Registered under {@code yog:callback_generator} in
 * {@code Registries.CHUNK_GENERATOR}; a dimension's JSON references it via
 * {@code "type": "yog:callback_generator", "generator_type_id": "..."}.
 *
 * <p>{@link #fillFromNoise} makes ONE native call per chunk
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
                    Codec.INT.fieldOf("height").forGetter(g -> g.height),
                    Codec.BOOL.optionalFieldOf("structures", true).forGetter(g -> g.structures),
                    Codec.BOOL.optionalFieldOf("features", true).forGetter(g -> g.features),
                    Codec.BOOL.optionalFieldOf("mob_spawning", true).forGetter(g -> g.mobSpawning))
            .apply(inst, YogCallbackChunkGenerator::new));

    /** The chunk currently being filled on this worker thread, if any. */
    private static final ThreadLocal<ChunkAccess> CURRENT_CHUNK = new ThreadLocal<>();

    private final String generatorTypeId;
    private final int minY;
    private final int height;

    private final boolean structures;
    private final boolean features;
    private final boolean mobSpawning;

    public YogCallbackChunkGenerator(
            BiomeSource biomeSource, String generatorTypeId, int minY, int height,
            boolean structures, boolean features, boolean mobSpawning) {
        super(biomeSource);
        this.generatorTypeId = generatorTypeId;
        this.minY = minY;
        this.height = height;
        this.structures = structures;
        this.features = features;
        this.mobSpawning = mobSpawning;
    }

    /** Register {@link #CODEC} under {@code yog:callback_generator}. Call once at mod-init. */
    public static void registerCodec() {
        Registry.register(BuiltInRegistries.CHUNK_GENERATOR, Identifier.fromNamespaceAndPath("yog", "callback_generator"), CODEC);
    }

    @Override
    protected MapCodec<? extends ChunkGenerator> codec() {
        return CODEC;
    }

    @Override
    public CompletableFuture<ChunkAccess> fillFromNoise(
            Blender blender, RandomState noiseConfig, StructureManager structureAccessor, ChunkAccess chunk) {
        net.minecraft.world.level.ChunkPos pos = chunk.getPos();
        CURRENT_CHUNK.set(chunk);
        try {
            NativeBridge.nativeGenerateChunk(generatorTypeId, pos.x(), pos.z(), minY, height);
        } finally {
            CURRENT_CHUNK.remove();
        }
        return CompletableFuture.completedFuture(chunk);
    }

    /**
     * Called by native code (via JNI, on the same thread that's inside
     * {@link #fillFromNoise}) once per block the mod's Rust closure places.
     * `x`/`z` are chunk-local (0..16); `y` is world-absolute.
     */
    public static boolean setBlockInGeneratingChunk(int x, int y, int z, String blockId) {
        ChunkAccess chunk = CURRENT_CHUNK.get();
        if (chunk == null) {
            return false;
        }
        Identifier ident = Identifier.tryParse(blockId);
        if (ident == null || !BuiltInRegistries.BLOCK.containsKey(ident)) {
            return false;
        }
        chunk.setBlockState(new BlockPos(x, y, z), BuiltInRegistries.BLOCK.getValue(ident).defaultBlockState(), 0);
        return true;
    }

    // ── Everything below is intentionally minimal — terrain shaping is the
    // registered closure's job via ChunkWriter::set_block, not this class's. ──

    @Override
    public void buildSurface(WorldGenRegion region, StructureManager structures, RandomState noiseConfig, ChunkAccess chunk) {
    }

    @Override
    public void applyCarvers(
            WorldGenRegion chunkRegion, long seed, RandomState noiseConfig, BiomeManager biomeAccess,
            StructureManager structureAccessor, ChunkAccess chunk) {
    }

    @Override
    public void spawnOriginalMobs(WorldGenRegion region) {
    }

    @Override
    public int getGenDepth() {
        return height;
    }

    @Override
    public int getSeaLevel() {
        return minY + height / 3;
    }

    @Override
    public int getMinY() {
        return minY;
    }

    @Override
    public int getBaseHeight(int x, int z, Heightmap.Types heightmap, LevelHeightAccessor world, RandomState noiseConfig) {
        return minY;
    }

    @Override
    public NoiseColumn getBaseColumn(int x, int z, LevelHeightAccessor world, RandomState noiseConfig) {
        return new NoiseColumn(minY, new BlockState[0]);
    }

    @Override
    public void addDebugScreenInfo(List<String> text, RandomState noiseConfig, BlockPos pos) {
    }

    @Override
    public void createStructures(net.minecraft.core.RegistryAccess registryManager,
            net.minecraft.world.level.chunk.ChunkGeneratorStructureState placementCalculator,
            StructureManager structureAccessor, ChunkAccess chunk,
            net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager structureTemplateManager,
            net.minecraft.resources.ResourceKey<net.minecraft.world.level.Level> level) {
        if (structures) {
            super.createStructures(registryManager, placementCalculator, structureAccessor, chunk, structureTemplateManager, level);
        }
    }

    @Override
    public void applyBiomeDecoration(net.minecraft.world.level.WorldGenLevel world, ChunkAccess chunk, StructureManager structureAccessor) {
        if (features) {
            super.applyBiomeDecoration(world, chunk, structureAccessor);
        }
    }

    @Override
    public net.minecraft.util.random.WeightedList<net.minecraft.world.level.biome.MobSpawnSettings.SpawnerData> getMobsAt(
            net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome> biome, StructureManager accessor,
            net.minecraft.world.entity.MobCategory group, BlockPos pos) {
        return mobSpawning ? super.getMobsAt(biome, accessor, group, pos) : net.minecraft.util.random.WeightedList.of();
    }
}
