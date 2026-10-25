package dev.yog.render;

import java.util.Set;
import net.minecraft.world.level.block.Block;

/**
 * The blocks this client draws, when a mod asked for only some
 * ({@code yog:client/only_blocks}): every other one isn't drawn and doesn't
 * hide what's behind it ({@link dev.yog.mixin.BlockFilterMixin}).
 */
public final class YogBlockFilter {
    private YogBlockFilter() {}

    /** The blocks drawn; none: every block. */
    private static volatile Set<Block> shown;

    public static void show(Set<Block> blocks) {
        shown = blocks == null || blocks.isEmpty() ? null : Set.copyOf(blocks);
    }

    /** Whether {@code block} is kept from being drawn. */
    public static boolean hidden(Block block) {
        Set<Block> s = shown;
        return s != null && !s.contains(block);
    }
}
