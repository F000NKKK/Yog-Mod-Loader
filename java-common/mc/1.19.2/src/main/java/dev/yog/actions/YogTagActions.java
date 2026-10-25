package dev.yog.actions;

import net.minecraft.core.Registry;
import dev.yog.YogServiceHost;

import net.minecraft.world.level.block.Block;
import net.minecraft.tags.TagKey;
import net.minecraft.world.item.Item;
import net.minecraft.resources.ResourceLocation;

/** The host's tag actions: the methods {@code yog_event::tag} names. */
public final class YogTagActions {
    private YogTagActions() {}

    @YogServiceHost
    public static boolean hasItemTag(String itemId, String tagId) {
        ResourceLocation iid = ResourceLocation.tryParse(itemId);
        ResourceLocation tid = ResourceLocation.tryParse(tagId);
        if (iid == null || tid == null || !Registry.ITEM.containsKey(iid)) return false;
        TagKey<Item> tag = TagKey.create(Registry.ITEM_REGISTRY, tid);
        return Registry.ITEM.get(iid).getDefaultInstance().is(tag);
    }

    @YogServiceHost
    public static boolean hasBlockTag(String blockId, String tagId) {
        ResourceLocation bid = ResourceLocation.tryParse(blockId);
        ResourceLocation tid = ResourceLocation.tryParse(tagId);
        if (bid == null || tid == null || !Registry.BLOCK.containsKey(bid)) return false;
        TagKey<Block> tag = TagKey.create(Registry.BLOCK_REGISTRY, tid);
        return Registry.BLOCK.get(bid).defaultBlockState().is(tag);
    }

    @YogServiceHost
    public static boolean hasEntityTag(String entityType, String tagId) {
        ResourceLocation eid = ResourceLocation.tryParse(entityType);
        ResourceLocation tid = ResourceLocation.tryParse(tagId);
        if (eid == null || tid == null || !Registry.ENTITY_TYPE.containsKey(eid)) return false;
        return Registry.ENTITY_TYPE.get(eid).is(TagKey.create(Registry.ENTITY_TYPE_REGISTRY, tid));
    }
}
