package dev.yog.content;

import net.minecraft.resources.Identifier;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;

/**
 * A living entity's attribute by its name: {@code max_health} is the game's
 * {@code max_health}, or {@code generic.max_health} before 1.21.2; a full id stays.
 */
public final class YogAttributes {
    private YogAttributes() {}

    /** {@code e}'s attribute {@code name}; null when it isn't living or hasn't it. */
    public static AttributeInstance of(Entity e, String name) {
        if (!(e instanceof LivingEntity le)) return null;
        for (String id : dev.yog.data.YogIds.attributeCandidates(name)) {
            Identifier key = Identifier.tryParse(id);
            if (key == null || !BuiltInRegistries.ATTRIBUTE.containsKey(key)) continue;
            Attribute a = BuiltInRegistries.ATTRIBUTE.getValue(key);
            return a == null ? null : le.getAttribute(BuiltInRegistries.ATTRIBUTE.wrapAsHolder(a));
        }
        return null;
    }
}
