package dev.yog.content;

import dev.yog.NativeBridge;

import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * A Yog block that gives off redstone power — how much, at each position,
 * its mod sets ({@link NativeBridge#setRedstoneOutput}); `"redstone": true`
 * in its .yblock.
 */
public class YogRedstoneBlock extends Block {
    public YogRedstoneBlock(Properties settings) {
        super(settings);
    }

    @Override
    public boolean isSignalSource(BlockState state) {
        return true;
    }

    @Override
    public int getSignal(BlockState state, BlockGetter world, BlockPos pos, Direction direction) {
        if (!(world instanceof Level w)) return 0;
        String dim = w.dimension().identifier().toString();
        return NativeBridge.REDSTONE_OUTPUT.getOrDefault(NativeBridge.redstoneKey(dim, pos.getX(), pos.getY(), pos.getZ()), 0);
    }
}
