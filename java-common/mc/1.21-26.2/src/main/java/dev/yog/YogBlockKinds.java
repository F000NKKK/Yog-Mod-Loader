package dev.yog;

import dev.yog.bus.YogTypes;
import dev.yog.data.YogContent;
import java.util.Map;
import java.util.function.Function;
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
import net.minecraft.world.level.block.state.properties.BlockSetType;
import net.minecraft.world.level.block.state.properties.WoodType;
import net.minecraft.world.level.material.PushReaction;

/**
 * The game's kinds of block a Yog block can be ({@code kind} of a
 * {@code .yblock}, yog-formats' {@code block_kinds.ydata}): each made of the
 * game's own class, by name — its states, shape and behaviour are the game's.
 */
final class YogBlockKinds {
    private YogBlockKinds() {}

    /** Makes a block of a kind: its settings, its definition, blocks by id (for {@code base}). */
    private interface Maker {
        Block make(BlockBehaviour.Properties settings, YogTypes.Block p, Function<String, Block> blocks);
    }

    private static final Map<String, Maker> KINDS = Map.of(
            "stairs", (s, p, blocks) -> new StairBlock(base(p, blocks), s) {},
            "slab", (s, p, blocks) -> new SlabBlock(s),
            "door", (s, p, blocks) -> new DoorBlock(blockSet(p), s.pushReaction(PushReaction.DESTROY)) {},
            "trapdoor", (s, p, blocks) -> new TrapDoorBlock(blockSet(p), s) {},
            "button", (s, p, blocks) -> new ButtonBlock(triggered(blockSet(p), p.mobsOnly()), p.pressTicks(),
                    s.pushReaction(PushReaction.DESTROY)) {},
            "pressure_plate", (s, p, blocks) -> new PressurePlateBlock(triggered(blockSet(p), p.mobsOnly()),
                    s.pushReaction(PushReaction.DESTROY)) {},
            "fence", (s, p, blocks) -> new FenceBlock(s),
            "fence_gate", (s, p, blocks) -> new FenceGateBlock(woodType(p), s));

    /** The block of {@code p}'s kind; {@code blocks} finds a block by id. */
    static Block make(BlockBehaviour.Properties settings, YogTypes.Block p, Function<String, Block> blocks) {
        Maker maker = KINDS.get(p.kind());
        if (maker == null) {
            YogLog.warn("block " + p.id() + ": no kind of block `" + p.kind() + "` — a plain block");
            return new Block(settings);
        }
        return maker.make(settings, p, blocks);
    }

    private static BlockState base(YogTypes.Block p, Function<String, Block> blocks) {
        Block base = p.base() == null ? null : blocks.apply(p.base());
        if (base == null) {
            YogLog.warn("block " + p.id() + ": no base block " + p.base());
            base = Blocks.AIR;
        }
        return base.defaultBlockState();
    }

    /** The registered block set of {@code p}'s {@code set}; oak by default. */
    private static BlockSetType blockSet(YogTypes.Block p) {
        String name = p.set() == null ? BlockSetType.OAK.name() : p.set();
        return BlockSetType.values().filter(t -> t.name().equals(name)).findFirst().orElseGet(() -> {
            YogLog.warn("block " + p.id() + ": no block set " + name + " — oak");
            return BlockSetType.OAK;
        });
    }

    /** The registered wood type of {@code p}'s {@code set}; oak by default. */
    private static WoodType woodType(YogTypes.Block p) {
        String name = p.set() == null ? WoodType.OAK.name() : p.set();
        return WoodType.values().filter(t -> t.name().equals(name)).findFirst().orElseGet(() -> {
            YogLog.warn("block " + p.id() + ": no wood type " + name + " — oak");
            return WoodType.OAK;
        });
    }

    /** {@code set}, set off by living things only or by everything (arrows press its buttons). */
    private static BlockSetType triggered(BlockSetType t, boolean mobsOnly) {
        BlockSetType.PressurePlateSensitivity rule = mobsOnly ? BlockSetType.PressurePlateSensitivity.MOBS : BlockSetType.PressurePlateSensitivity.EVERYTHING;
        if (t.canButtonBeActivatedByArrows() == !mobsOnly && t.pressurePlateSensitivity() == rule) return t;
        return new BlockSetType(t.name(), t.canOpenByHand(), t.canOpenByWindCharge(), !mobsOnly, rule, t.soundType(),
                t.doorClose(), t.doorOpen(), t.trapdoorClose(), t.trapdoorOpen(), t.pressurePlateClickOff(),
                t.pressurePlateClickOn(), t.buttonClickOff(), t.buttonClickOn());
    }
}
