package dev.yog;

import java.util.List;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import org.jetbrains.annotations.Nullable;

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

    @Override
    public Component getDescription() {
        return displayName == null || displayName.isEmpty()
                ? super.getDescription()
                : Component.literal(displayName);
    }

    @Override
    public void appendHoverText(ItemStack stack, @Nullable Level level, List<Component> lines, TooltipFlag flag) {
        super.appendHoverText(stack, level, lines, flag);
        String descKey = this.getDescriptionId() + ".desc";
        String resolved = net.minecraft.client.resources.language.I18n.exists(descKey)
                ? net.minecraft.client.resources.language.I18n.get(descKey)
                : (tooltip != null && !tooltip.isEmpty() ? tooltip : null);
        if (resolved != null) {
            for (String line : resolved.split("\n")) {
                lines.add(Component.literal(line));
            }
        }
    }
}
