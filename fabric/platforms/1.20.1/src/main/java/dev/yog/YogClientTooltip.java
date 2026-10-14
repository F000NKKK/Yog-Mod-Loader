package dev.yog;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.tooltip.TooltipComponent;

/** Draws a {@link YogTooltipData}: the mod's code paints it through the runtime. */
public record YogClientTooltip(YogTooltipData data) implements TooltipComponent {
    @Override
    public int getHeight() {
        return data.height() + 2;
    }

    @Override
    public int getWidth(TextRenderer font) {
        return data.width();
    }

    @Override
    public void drawItems(TextRenderer font, int x, int y, DrawContext ctx) {
        ctx.draw();
        var window = MinecraftClient.getInstance().getWindow();
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeTooltipDraw(data.item(), data.snbt(), x, y, data.width(), data.height(),
                window.getScaledWidth(), window.getScaledHeight());
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState();
    }
}
