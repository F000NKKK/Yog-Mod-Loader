package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResultHolder;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;

/** A Yog item that opens a book GUI when right-clicked. */
public class YogBookItem extends YogItem {
    private final String bookId;

    public YogBookItem(Properties properties, String displayName, String tooltip, String bookId) {
        super(properties, displayName, tooltip);
        this.bookId = bookId;
    }

    @Override
    public InteractionResultHolder<ItemStack> use(Level level, Player player, InteractionHand hand) {
        if (level.isClientSide) {
            String json = NativeBridge.nativeBookJson(bookId);
            if (json != null && !json.equals("null")) {
                Minecraft.getInstance().setScreen(new YogUIScreen(bookId));
            }
        }
        return InteractionResultHolder.success(player.getItemInHand(hand));
    }
}
