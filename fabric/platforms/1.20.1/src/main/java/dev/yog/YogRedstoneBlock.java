package dev.yog;

import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.world.BlockView;
import net.minecraft.world.World;

/**
 * A Yog block that gives off redstone power — how much, at each position,
 * its mod sets ({@link NativeBridge#setRedstoneOutput}); `"redstone": true`
 * in its .yblock.
 */
public class YogRedstoneBlock extends Block {
    public YogRedstoneBlock(Settings settings) {
        super(settings);
    }

    @Override
    public boolean emitsRedstonePower(BlockState state) {
        return true;
    }

    @Override
    public int getWeakRedstonePower(BlockState state, BlockView world, BlockPos pos, Direction direction) {
        if (!(world instanceof World w)) return 0;
        String dim = w.getRegistryKey().getValue().toString();
        return NativeBridge.REDSTONE_OUTPUT.getOrDefault(NativeBridge.redstoneKey(dim, pos.getX(), pos.getY(), pos.getZ()), 0);
    }
}
