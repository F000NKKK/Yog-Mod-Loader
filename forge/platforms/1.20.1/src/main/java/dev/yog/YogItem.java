package dev.yog;

import java.util.List;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.level.Level;
import org.jetbrains.annotations.Nullable;

/** An item whose display name and tooltip come from a Yog mod (no lang file needed). */
public class YogItem extends Item {
    private final String displayName;
    private final String tooltip;

    public YogItem(Properties properties, String displayName, String tooltip) {
        super(properties);
        this.displayName = displayName;
        this.tooltip = tooltip;
    }

    @Override
    public Component getName(ItemStack stack) {
        String[] own = ownText(stack);
        if (own != null && !own[0].isEmpty()) return Component.literal(own[0]);
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
        // A mod's own lines for this stack (a written blueprint, a fabricated
        // chip) stand instead of the item's description.
        String[] own = ownText(stack);
        if (own != null) {
            for (int i = 1; i < own.length; i++) lines.add(Component.literal(own[i]));
            return;
        }
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

    /** A Yog mod's picture under the tooltip, if it gives this stack one. */
    @Override
    public java.util.Optional<net.minecraft.world.inventory.tooltip.TooltipComponent> getTooltipImage(net.minecraft.world.item.ItemStack stack) {
        return YogTooltipData.of(stack);
    }

    /** A Yog mod's name (first) and tooltip lines for this stack ({@code Registry::on_item_text}), or null. */
    static String[] ownText(ItemStack stack) {
        try {
            String t = NativeBridge.nativeItemText(net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(stack.getItem()).toString(), NativeBridge.tooltipSnbt(stack));
            return t == null ? null : t.split("\n", -1);
        } catch (UnsatisfiedLinkError e) {
            return null;
        }
    }
}
