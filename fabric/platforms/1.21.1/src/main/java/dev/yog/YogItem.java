package dev.yog;

import java.util.List;
import net.minecraft.item.tooltip.TooltipType;
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
        String[] own = ownText(stack);
        if (own != null && !own[0].isEmpty()) return Text.literal(own[0]);
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
    public void appendTooltip(ItemStack stack, TooltipContext context, List<Text> tooltipLines, TooltipType type) {
        super.appendTooltip(stack, context, tooltipLines, type);
        String descKey = this.getTranslationKey() + ".desc";
        String resolved = net.minecraft.client.resource.language.I18n.hasTranslation(descKey)
                ? net.minecraft.client.resource.language.I18n.translate(descKey)
                : (tooltip != null && !tooltip.isEmpty() ? tooltip : null);
        if (resolved != null) {
            for (String line : resolved.split("\n")) {
                tooltipLines.add(Text.literal(line));
            }
        }
        String[] own = ownText(stack);
        if (own != null) for (int i = 1; i < own.length; i++) tooltipLines.add(Text.literal(own[i]));
    }

    /** A Yog mod's picture under the tooltip, if it gives this stack one. */
    @Override
    public java.util.Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(net.minecraft.item.ItemStack stack) {
        return YogTooltipData.of(stack);
    }

    /** A Yog mod's name (first) and tooltip lines for this stack ({@code Registry::on_item_text}), or null. */
    static String[] ownText(ItemStack stack) {
        try {
            String t = NativeBridge.nativeItemText(net.minecraft.registry.Registries.ITEM.getId(stack.getItem()).toString(), NativeBridge.tooltipSnbt(stack));
            return t == null ? null : t.split("\n", -1);
        } catch (UnsatisfiedLinkError e) {
            return null;
        }
    }
}
