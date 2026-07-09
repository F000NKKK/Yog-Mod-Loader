package dev.yog;

import java.util.List;
import net.minecraft.block.Block;
import net.minecraft.client.item.TooltipContext;
import net.minecraft.item.BlockItem;
import net.minecraft.item.ItemStack;
import net.minecraft.text.Text;
import net.minecraft.world.World;
import org.jetbrains.annotations.Nullable;

/** A block's item form whose display name and tooltip come from a Yog mod. */
public class YogBlockItem extends BlockItem {
    private final String displayName;
    private final String tooltip;

    public YogBlockItem(Block block, Settings settings, String displayName, String tooltip) {
        super(block, settings);
        this.displayName = displayName;
        this.tooltip = tooltip;
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

    @Override
    public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, TooltipContext ctx) {
        String descKey = this.getTranslationKey() + ".desc";
        String resolved = net.minecraft.client.resource.language.I18n.hasTranslation(descKey)
                ? net.minecraft.client.resource.language.I18n.translate(descKey)
                : (tooltip != null && !tooltip.isEmpty() ? tooltip : null);
        if (resolved != null) {
            for (String line : resolved.split("\n")) {
                lines.add(Text.literal(line));
            }
        }
    }
}
