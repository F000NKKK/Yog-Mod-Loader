package dev.yog;

import net.minecraft.network.chat.Component;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;

/** A block's item form whose display name comes from a Yog mod. */
public class YogBlockItem extends BlockItem {
    private final String displayName;

    public YogBlockItem(Block block, Properties properties, String displayName) {
        super(block, properties);
        this.displayName = displayName;
    }

    @Override
    public Component getName(ItemStack stack) {
        return displayName == null || displayName.isEmpty()
                ? super.getName(stack)
                : Component.literal(displayName);
    }

    @Override
    public Component getDescription() {
        return displayName == null || displayName.isEmpty()
                ? super.getDescription()
                : Component.literal(displayName);
    }
}
