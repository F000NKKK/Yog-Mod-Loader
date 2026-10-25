package dev.yog.content.blocks;

import dev.yog.YogLog;
import dev.yog.YogNames;

import dev.yog.bus.YogTypes;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.function.Function;
import java.util.stream.Stream;
import net.minecraft.core.Registry;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundEvents;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.ButtonBlock;
import net.minecraft.world.level.block.DoorBlock;
import net.minecraft.world.level.block.FenceBlock;
import net.minecraft.world.level.block.FenceGateBlock;
import net.minecraft.world.level.block.PressurePlateBlock;
import net.minecraft.world.level.block.SlabBlock;
import net.minecraft.world.level.block.StairBlock;
import net.minecraft.world.level.block.TrapDoorBlock;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.WoodType;
import net.minecraft.world.level.material.Material;

/**
 * The game's kinds of block a Yog block can be ({@code kind} of a
 * {@code .yblock}, yog-formats' {@code block_kinds.ydata}): each made of the
 * game's own class, by name — its states, shape and behaviour are the game's.
 * This version has no block sets: a kind's {@code set} picks its material
 * (what opens by hand, how it sounds), read off the set's own vanilla block.
 */
public final class YogBlockKinds {
    private YogBlockKinds() {}

    /** Makes a block of a kind: its settings, its definition, blocks by id (for {@code base}). */
    private interface Maker {
        public Block make(BlockBehaviour.Properties settings, YogTypes.Block p, Function<String, Block> blocks);
    }

    private static final String DOOR = "door", TRAPDOOR = "trapdoor", BUTTON = "button",
            PRESSURE_PLATE = "pressure_plate", FENCE_GATE = "fence_gate";

    private static final Map<String, Maker> KINDS = Map.of(
            "stairs", (s, p, blocks) -> new StairBlock(base(p, blocks), s) {},
            "slab", (s, p, blocks) -> new SlabBlock(s),
            DOOR, (s, p, blocks) -> new DoorBlock(s) {},
            TRAPDOOR, (s, p, blocks) -> new TrapDoorBlock(s) {},
            // Arrows press a button set off by everything; how long it stays
            // pressed is this version's own (it has no setting for it).
            BUTTON, (s, p, blocks) -> button(s, !p.mobsOnly(), material(p) == Material.WOOD),
            PRESSURE_PLATE, (s, p, blocks) -> new PressurePlateBlock(p.mobsOnly()
                    ? PressurePlateBlock.Sensitivity.MOBS : PressurePlateBlock.Sensitivity.EVERYTHING, s) {},
            "fence", (s, p, blocks) -> new FenceBlock(s),
            FENCE_GATE, (s, p, blocks) -> new FenceGateBlock(s));

    /** The kinds a {@code set} applies to, the order its vanilla block is looked for in. */
    private static final List<String> SET_KINDS = List.of(DOOR, BUTTON, PRESSURE_PLATE, TRAPDOOR, FENCE_GATE);

    /** The block of {@code p}'s kind; {@code blocks} finds a block by id. */
    public static Block make(BlockBehaviour.Properties settings, YogTypes.Block p, Function<String, Block> blocks) {
        Maker maker = KINDS.get(p.kind());
        if (maker == null) {
            YogLog.warn("block " + p.id() + ": no kind of block `" + p.kind() + "` — a plain block");
            return new Block(settings);
        }
        return maker.make(settings, p, blocks);
    }

    /**
     * What {@code p} is made of: for a kind with a set, the material of the
     * set's vanilla block of that kind ({@code iron_door}), else of another
     * kind ({@code stone_button}); stone for every other block.
     */
    public static Material material(YogTypes.Block p) {
        if (!SET_KINDS.contains(p.kind())) return Material.STONE;
        String set = p.set() == null ? WoodType.OAK.name() : p.set();
        return Stream.concat(Stream.of(p.kind()), SET_KINDS.stream())
                .map(kind -> ResourceLocation.tryParse(set + YogNames.Sep.WORD + kind))
                .map(id -> id == null ? Optional.<Block>empty() : Registry.BLOCK.getOptional(id))
                .flatMap(Optional::stream)
                .findFirst()
                .map(b -> b.defaultBlockState().getMaterial())
                .orElseGet(() -> {
                    YogLog.warn("block " + p.id() + ": no block set " + set + " — wood");
                    return Material.WOOD;
                });
    }

    private static Block button(BlockBehaviour.Properties s, boolean arrows, boolean wooden) {
        SoundEvent on = wooden ? SoundEvents.WOODEN_BUTTON_CLICK_ON : SoundEvents.STONE_BUTTON_CLICK_ON;
        SoundEvent off = wooden ? SoundEvents.WOODEN_BUTTON_CLICK_OFF : SoundEvents.STONE_BUTTON_CLICK_OFF;
        return new ButtonBlock(arrows, s) {
            @Override protected SoundEvent getSound(boolean pressed) { return pressed ? on : off; }
        };
    }

    private static BlockState base(YogTypes.Block p, Function<String, Block> blocks) {
        Block base = p.base() == null ? null : blocks.apply(p.base());
        if (base == null) {
            YogLog.warn("block " + p.id() + ": no base block " + p.base());
            base = Blocks.AIR;
        }
        return base.defaultBlockState();
    }
}
