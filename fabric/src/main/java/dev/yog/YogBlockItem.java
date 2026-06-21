package dev.yog;

import net.minecraft.block.Block;
import net.minecraft.item.BlockItem;
import net.minecraft.item.ItemStack;
import net.minecraft.text.Text;

/** A block's item form whose display name comes from a Yog mod. */
public class YogBlockItem extends BlockItem {
    private final String displayName;

    public YogBlockItem(Block block, Settings settings, String displayName) {
        super(block, settings);
        this.displayName = displayName;
    }

    @Override
    public Text getName(ItemStack stack) {
        return displayName == null || displayName.isEmpty()
                ? super.getName(stack)
                : Text.literal(displayName);
    }

    @Override
    public Text getName() {
        return displayName == null || displayName.isEmpty()
                ? super.getName()
                : Text.literal(displayName);
    }
}
