package dev.yog.actions;

import dev.yog.YogServiceHost;

import net.minecraft.world.level.block.Block;
import net.minecraft.tags.TagKey;
import net.minecraft.world.item.Item;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;

/** The host's tag actions: the methods {@code yog_event::tag} names. */
public final class YogTagActions {
    private YogTagActions() {}

    @YogServiceHost
    public static boolean hasItemTag(String itemId, String tagId) {
        ResourceLocation iid = ResourceLocation.tryParse(itemId);
        ResourceLocation tid = ResourceLocation.tryParse(tagId);
        if (iid == null || tid == null || !BuiltInRegistries.ITEM.containsKey(iid)) return false;
        TagKey<Item> tag = TagKey.create(Registries.ITEM, tid);
        return BuiltInRegistries.ITEM.get(iid).getDefaultInstance().is(tag);
    }

    @YogServiceHost
    public static boolean hasBlockTag(String blockId, String tagId) {
        ResourceLocation bid = ResourceLocation.tryParse(blockId);
        ResourceLocation tid = ResourceLocation.tryParse(tagId);
        if (bid == null || tid == null || !BuiltInRegistries.BLOCK.containsKey(bid)) return false;
        TagKey<Block> tag = TagKey.create(Registries.BLOCK, tid);
        return BuiltInRegistries.BLOCK.get(bid).defaultBlockState().is(tag);
    }

    @YogServiceHost
    public static boolean hasEntityTag(String entityType, String tagId) {
        ResourceLocation eid = ResourceLocation.tryParse(entityType);
        ResourceLocation tid = ResourceLocation.tryParse(tagId);
        if (eid == null || tid == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(eid)) return false;
        return BuiltInRegistries.ENTITY_TYPE.get(eid).is(TagKey.create(Registries.ENTITY_TYPE, tid));
    }
}
