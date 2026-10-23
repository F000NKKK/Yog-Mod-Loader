package dev.yog.content;

import dev.yog.NativeBridge;
import dev.yog.YogNames;

import dev.yog.util.YogGeneratorSettings;

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
                    BiomeSource.CODEC.fieldOf(YogNames.CallbackGenerator.BIOME_SOURCE).forGetter(g -> g.biomeSource),
                    YogGeneratorSettings.MAP_CODEC.forGetter(g -> g.settings))
            .apply(inst, YogCallbackChunkGenerator::new));

    /** The chunk currently being filled on this worker thread, if any. */
    private static final ThreadLocal<ChunkAccess> CURRENT_CHUNK = new ThreadLocal<>();

    /** Its generator in code, height, and what it has. */
    private final YogGeneratorSettings settings;

    public YogCallbackChunkGenerator(BiomeSource biomeSource, YogGeneratorSettings settings) {
        super(biomeSource);
        this.settings = settings;
    }

    /** Register {@link #CODEC} under {@code yog:callback_generator}. Call once at mod-init. */
    /** Its codec, with {@link YogRegistrar}: dimensions name the generator {@code yog:callback_generator}. */
    public static void schedule() {
        YogRegistrar.add(net.minecraft.core.registries.Registries.CHUNK_GENERATOR, sink -> sink.register(
                net.minecraft.core.registries.Registries.CHUNK_GENERATOR, Identifier.fromNamespaceAndPath(YogNames.Ns.YOG, YogNames.CallbackGenerator.PATH), CODEC));
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
            NativeBridge.nativeGenerateChunk(settings.generatorTypeId(), pos.x(), pos.z(), settings.minY(), settings.height());
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
        return settings.height();
    }

    @Override
    public int getSeaLevel() {
        return settings.minY() + settings.height() / 3;
    }

    @Override
    public int getMinY() {
        return settings.minY();
    }

    @Override
    public int getBaseHeight(int x, int z, Heightmap.Types heightmap, LevelHeightAccessor world, RandomState noiseConfig) {
        return settings.minY();
    }

    @Override
    public NoiseColumn getBaseColumn(int x, int z, LevelHeightAccessor world, RandomState noiseConfig) {
        return new NoiseColumn(settings.minY(), new BlockState[0]);
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
        if (settings.structures()) {
            super.createStructures(registryManager, placementCalculator, structureAccessor, chunk, structureTemplateManager, level);
        }
    }

    @Override
    public void applyBiomeDecoration(net.minecraft.world.level.WorldGenLevel world, ChunkAccess chunk, StructureManager structureAccessor) {
        if (settings.features()) {
            super.applyBiomeDecoration(world, chunk, structureAccessor);
        }
    }

    @Override
    public net.minecraft.util.random.WeightedList<net.minecraft.world.level.biome.MobSpawnSettings.SpawnerData> getMobsAt(
            net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome> biome, StructureManager accessor,
            net.minecraft.world.entity.MobCategory group, BlockPos pos) {
        return settings.mobSpawning() ? super.getMobsAt(biome, accessor, group, pos) : net.minecraft.util.random.WeightedList.of();
    }
}
