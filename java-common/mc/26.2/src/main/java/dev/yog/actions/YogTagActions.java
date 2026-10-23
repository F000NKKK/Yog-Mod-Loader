package dev.yog.actions;

import dev.yog.YogServiceHost;

import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.tags.TagKey;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.Block;

/** The host's tag actions: the methods {@code yog_event::tag} names. */
public final class YogTagActions {
    private YogTagActions() {}

    @YogServiceHost
    public static boolean hasItemTag(String itemId, String tagId) {
        Identifier iid = Identifier.tryParse(itemId);
        Identifier tid = Identifier.tryParse(tagId);
        if (iid == null || tid == null || !BuiltInRegistries.ITEM.containsKey(iid)) return false;
        TagKey<Item> tag = TagKey.create(Registries.ITEM, tid);
        return BuiltInRegistries.ITEM.getValue(iid).getDefaultInstance().is(tag);
    }

    @YogServiceHost
    public static boolean hasBlockTag(String blockId, String tagId) {
        Identifier bid = Identifier.tryParse(blockId);
        Identifier tid = Identifier.tryParse(tagId);
        if (bid == null || tid == null || !BuiltInRegistries.BLOCK.containsKey(bid)) return false;
        TagKey<Block> tag = TagKey.create(Registries.BLOCK, tid);
        return BuiltInRegistries.BLOCK.getValue(bid).defaultBlockState().is(tag);
    }

    @YogServiceHost
    public static boolean hasEntityTag(String entityType, String tagId) {
        Identifier eid = Identifier.tryParse(entityType);
        Identifier tid = Identifier.tryParse(tagId);
        if (eid == null || tid == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(eid)) return false;
        return BuiltInRegistries.ENTITY_TYPE.get(eid).map(h -> h.is(TagKey.create(Registries.ENTITY_TYPE, tid))).orElse(false);
    }
}
