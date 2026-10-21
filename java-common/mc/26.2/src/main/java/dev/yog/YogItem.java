package dev.yog;

import dev.yog.bus.YogServices;
import dev.yog.bus.YogTypes;
import java.util.function.Consumer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;

/** An item whose display name and tooltip come from a Yog mod (no lang file needed). */
public class YogItem extends Item implements YogTooltipLines {
    private final String displayName;
    private final String tooltip;

    public YogItem(Properties settings, String displayName, String tooltip) {
        super(settings);
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
    public void tooltipLines(ItemStack stack, Consumer<Component> tooltipLines) {
        // A mod's own lines for this stack (a written blueprint, a fabricated
        // chip) stand instead of the item's description.
        String[] own = ownText(stack);
        if (own != null) {
            for (int i = 1; i < own.length; i++) tooltipLines.accept(Component.literal(own[i]));
            return;
        }
        YogTools.hover(this, tooltip, tooltipLines);
    }

    /** A Yog mod's picture under the tooltip, if it gives this stack one. */
    @Override
    public java.util.Optional<net.minecraft.world.inventory.tooltip.TooltipComponent> getTooltipImage(net.minecraft.world.item.ItemStack stack) {
        return YogTooltipData.of(stack);
    }

    /** A Yog mod's name (first) and tooltip lines for this stack ({@code Registry::on_item_text}, the {@code item/text} service), or null. */
    static String[] ownText(ItemStack stack) {
        try {
            YogTypes.Shown shown = new YogServices.Item.Text(net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(stack.getItem()).toString(), NativeBridge.tooltipSnbt(stack)).call();
            if (shown == null) return null;
            String[] out = new String[1 + shown.lines().size()];
            out[0] = shown.name() == null ? "" : shown.name();
            for (int i = 0; i < shown.lines().size(); i++) out[i + 1] = shown.lines().get(i);
            return out;
        } catch (UnsatisfiedLinkError e) {
            return null;
        }
    }
}
