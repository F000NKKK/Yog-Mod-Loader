package dev.yog.content;

import dev.yog.actions.YogUiActions;

import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;

/** A Yog item that opens a book GUI when right-clicked. */
public class YogBookItem extends YogItem {
    private final String bookId;

    public YogBookItem(Properties settings, String displayName, String tooltip, String bookId) {
        super(settings, displayName, tooltip);
        this.bookId = bookId;
    }

    @Override
    public InteractionResult use(Level world, Player player, InteractionHand hand) {
        if (world.isClientSide()) {
            YogUiActions.openUI(bookId, true, false);
        }
        return InteractionResult.SUCCESS;
    }
}