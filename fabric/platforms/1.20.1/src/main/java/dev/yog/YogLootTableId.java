package dev.yog;

/** A loot table's id, stamped on it where it's looked up (yarn's {@code LootTable} has none). */
public interface YogLootTableId {
    String yog$id();

    void yog$setId(String id);
}
