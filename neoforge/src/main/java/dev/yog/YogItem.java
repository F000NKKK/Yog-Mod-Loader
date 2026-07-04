package dev.yog;

import java.util.List;
import net.minecraft.client.item.TooltipContext;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.text.Text;
import net.minecraft.world.World;
import org.jetbrains.annotations.Nullable;

/** An item whose display name and tooltip come from a Yog mod (no lang file needed). */
public class YogItem extends Item {
    private final String displayName;
    private final String tooltip;

    public YogItem(Settings settings, String displayName, String tooltip) {
        super(settings);
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
        if (tooltip != null && !tooltip.isEmpty()) {
            lines.add(Text.literal(tooltip));
        }
    }
}
