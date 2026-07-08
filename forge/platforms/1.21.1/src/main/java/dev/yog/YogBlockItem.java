package dev.yog;

import java.util.List;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.level.block.Block;

/** A block's item form whose display name and tooltip come from a Yog mod. */
public class YogBlockItem extends BlockItem {
    private final String displayName;
    private final String tooltip;

    public YogBlockItem(Block block, Properties properties, String displayName, String tooltip) {
        super(block, properties);
        this.displayName = displayName;
        this.tooltip = tooltip;
    }

    @Override
    public Component getName(ItemStack stack) {
        return displayName == null || displayName.isEmpty()
                ? super.getName(stack)
                : Component.literal(displayName);
    }

    /** Return the custom display name (same as getName) — kept for API consistency across versions. */
    public Component getDescription() {
        return displayName == null || displayName.isEmpty()
                ? getName(ItemStack.EMPTY)
                : Component.literal(displayName);
    }

    @Override
    public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
        super.appendHoverText(stack, context, lines, flag);
        if (tooltip != null && !tooltip.isEmpty()) {
            lines.add(Component.literal(tooltip));
        }
    }
}
