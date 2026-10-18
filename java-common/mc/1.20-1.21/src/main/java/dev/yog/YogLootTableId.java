package dev.yog;

/** A loot table's id, stamped on it where it's looked up ({@code mixin.LootLookupMixin}): the game's table has none. */
public interface YogLootTableId {
    String yog$id();

    void yog$setId(String id);
}
