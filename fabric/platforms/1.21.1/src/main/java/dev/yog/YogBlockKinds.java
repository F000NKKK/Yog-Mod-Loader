package dev.yog;

import dev.yog.data.YogContent;
import java.util.Map;
import java.util.function.Function;
import net.minecraft.block.AbstractBlock;
import net.minecraft.block.Block;
import net.minecraft.block.BlockSetType;
import net.minecraft.block.BlockState;
import net.minecraft.block.Blocks;
import net.minecraft.block.ButtonBlock;
import net.minecraft.block.DoorBlock;
import net.minecraft.block.FenceBlock;
import net.minecraft.block.FenceGateBlock;
import net.minecraft.block.PressurePlateBlock;
import net.minecraft.block.SlabBlock;
import net.minecraft.block.StairsBlock;
import net.minecraft.block.TrapdoorBlock;
import net.minecraft.block.WoodType;
import net.minecraft.block.piston.PistonBehavior;

/**
 * The game's kinds of block a Yog block can be ({@code kind} of a
 * {@code .yblock}, yog-formats' {@code block_kinds.ydata}): each made of the
 * game's own class, by name — its states, shape and behaviour are the game's.
 */
final class YogBlockKinds {
    private YogBlockKinds() {}

    /** Makes a block of a kind: its settings, its definition, blocks by id (for {@code base}). */
    private interface Maker {
        Block make(AbstractBlock.Settings settings, YogContent.Block p, Function<String, Block> blocks);
    }

    private static final Map<String, Maker> KINDS = Map.of(
            "stairs", (s, p, blocks) -> new StairsBlock(base(p, blocks), s) {},
            "slab", (s, p, blocks) -> new SlabBlock(s),
            "door", (s, p, blocks) -> new DoorBlock(blockSet(p), s.pistonBehavior(PistonBehavior.DESTROY)) {},
            "trapdoor", (s, p, blocks) -> new TrapdoorBlock(blockSet(p), s) {},
            "button", (s, p, blocks) -> new ButtonBlock(triggered(blockSet(p), p.mobsOnly()), p.pressTicks(),
                    s.pistonBehavior(PistonBehavior.DESTROY)) {},
            "pressure_plate", (s, p, blocks) -> new PressurePlateBlock(triggered(blockSet(p), p.mobsOnly()),
                    s.pistonBehavior(PistonBehavior.DESTROY)) {},
            "fence", (s, p, blocks) -> new FenceBlock(s),
            "fence_gate", (s, p, blocks) -> new FenceGateBlock(woodType(p), s));

    /** The block of {@code p}'s kind; {@code blocks} finds a block by id. */
    static Block make(AbstractBlock.Settings settings, YogContent.Block p, Function<String, Block> blocks) {
        Maker maker = KINDS.get(p.kind());
        if (maker == null) {
            YogLog.warn("block " + p.id() + ": no kind of block `" + p.kind() + "` — a plain block");
            return new Block(settings);
        }
        return maker.make(settings, p, blocks);
    }

    private static BlockState base(YogContent.Block p, Function<String, Block> blocks) {
        Block base = p.base() == null ? null : blocks.apply(p.base());
        if (base == null) {
            YogLog.warn("block " + p.id() + ": no base block " + p.base());
            base = Blocks.AIR;
        }
        return base.getDefaultState();
    }

    /** The registered block set of {@code p}'s {@code set}; oak by default. */
    private static BlockSetType blockSet(YogContent.Block p) {
        String name = p.set() == null ? BlockSetType.OAK.name() : p.set();
        return BlockSetType.stream().filter(t -> t.name().equals(name)).findFirst().orElseGet(() -> {
            YogLog.warn("block " + p.id() + ": no block set " + name + " — oak");
            return BlockSetType.OAK;
        });
    }

    /** The registered wood type of {@code p}'s {@code set}; oak by default. */
    private static WoodType woodType(YogContent.Block p) {
        String name = p.set() == null ? WoodType.OAK.name() : p.set();
        return WoodType.stream().filter(t -> t.name().equals(name)).findFirst().orElseGet(() -> {
            YogLog.warn("block " + p.id() + ": no wood type " + name + " — oak");
            return WoodType.OAK;
        });
    }

    /** {@code set}, set off by living things only or by everything (arrows press its buttons). */
    private static BlockSetType triggered(BlockSetType t, boolean mobsOnly) {
        BlockSetType.ActivationRule rule = mobsOnly ? BlockSetType.ActivationRule.MOBS : BlockSetType.ActivationRule.EVERYTHING;
        if (t.canButtonBeActivatedByArrows() == !mobsOnly && t.pressurePlateSensitivity() == rule) return t;
        return new BlockSetType(t.name(), t.canOpenByHand(), t.canOpenByWindCharge(), !mobsOnly, rule, t.soundType(),
                t.doorClose(), t.doorOpen(), t.trapdoorClose(), t.trapdoorOpen(), t.pressurePlateClickOff(),
                t.pressurePlateClickOn(), t.buttonClickOff(), t.buttonClickOn());
    }
}
