package dev.yog;

import java.util.function.Consumer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;

/** A block's item form whose display name and tooltip come from a Yog mod. */
public class YogBlockItem extends BlockItem implements YogTooltipLines {
    private final String displayName;
    private final String tooltip;

    public YogBlockItem(Block block, Properties settings, String displayName, String tooltip) {
        super(block, settings);
        this.displayName = displayName;
        this.tooltip = tooltip;
    }

    @Override
    public Component getName(ItemStack stack) {
        return displayName == null || displayName.isEmpty()
                ? super.getName(stack)
                : Component.literal(displayName);
    }


    @Override
    public void tooltipLines(ItemStack stack, Consumer<Component> tooltipLines) {
        YogTools.hover(this, tooltip, tooltipLines);
    }

    /** A Yog mod's picture under the tooltip, if it gives this stack one. */
    @Override
    public java.util.Optional<net.minecraft.world.inventory.tooltip.TooltipComponent> getTooltipImage(net.minecraft.world.item.ItemStack stack) {
        return YogTooltipData.of(stack);
    }
}
