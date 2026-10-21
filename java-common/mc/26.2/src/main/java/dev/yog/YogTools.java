package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.world.item.component.ItemAttributeModifiers;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.core.component.DataComponents;
import java.util.EnumMap;
import java.util.Optional;
import java.util.function.Consumer;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.locale.Language;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.tags.BlockTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.ToolMaterial;
import net.minecraft.world.item.equipment.ArmorMaterial;
import net.minecraft.world.item.equipment.ArmorType;
import net.minecraft.world.item.equipment.EquipmentAsset;
import net.minecraft.world.item.equipment.EquipmentAssets;
import net.minecraft.world.level.block.Block;

/**
 * Tools, weapons and armor of a material (the game's or a mod's), as the
 * runtime describes them ({@link YogContent.Tool}, {@link YogContent.Armor}):
 * since 1.21.2 a tool or a piece of armor is a plain item whose components
 * (tool rules, attributes, equippable) say what it is.
 */
public final class YogTools {
    private YogTools() {}

    /** A tag prefix in a repair ingredient ({@code #minecraft:planks}). */
    private static final String TAG_MARK = "#";
    /** The suffix of an item's translation key that gives its description. */
    private static final String DESC_SUFFIX = ".desc";

    /** The tool or armor item {@code p} says, or null for neither. */
    public static Item make(Item.Properties s, YogContent.Item p) {
        if (p.tool() != null) {
            YogContent.Tool t = p.tool().resolved(YogTools::gameAttack);
            ToolMaterial m = material(t.material());
            float damage = t.attackDamage(), speed = t.attackSpeed();
            Item.Properties tool = switch (t.kind()) {
                case YogNames.ToolKind.PICKAXE -> s.pickaxe(m, damage, speed);
                case YogNames.ToolKind.AXE -> s.axe(m, damage, speed);
                case YogNames.ToolKind.SHOVEL -> s.shovel(m, damage, speed);
                case YogNames.ToolKind.HOE -> s.hoe(m, damage, speed);
                case YogNames.ToolKind.SWORD -> s.sword(m, damage, speed);
                default -> s.tool(m, mineable(t.kind()), damage, speed, 0.0F);
            };
            return new Named(repairable(tool, t.material().repair()), p.name(), p.tooltip());
        }
        if (p.armor() != null) {
            YogContent.Armor a = p.armor();
            YogContent.ArmorMaterial m = a.material();
            EnumMap<ArmorType, Integer> defense = new EnumMap<>(ArmorType.class);
            defense.put(ArmorType.HELMET, m.head());
            defense.put(ArmorType.CHESTPLATE, m.chest());
            defense.put(ArmorType.LEGGINGS, m.legs());
            defense.put(ArmorType.BOOTS, m.feet());
            defense.put(ArmorType.BODY, m.chest());
            ArmorMaterial material = new ArmorMaterial(m.durability(), defense, m.enchantability(),
                    BuiltInRegistries.SOUND_EVENT.wrapAsHolder(YogContentRegistry.soundEvent(m.equipSound())),
                    m.toughness(), m.knockbackResistance(), repairTag(m.repair()), asset(m.texture()));
            Item.Properties armor = s.humanoidArmor(material, armorType(a.slot()));
            return new Named(repairable(armor, m.repair()), p.name(), p.tooltip());
        }
        return null;
    }

    /** Game item {@code id}'s main-hand attack damage and speed modifiers; null for no such item. */
    static float[] gameAttack(String id) {
        Identifier key = Identifier.tryParse(id);
        Item item = key == null ? Items.AIR : BuiltInRegistries.ITEM.getValue(key);
        if (item == Items.AIR) return null;
        float[] attack = new float[2];
        item.components().getOrDefault(DataComponents.ATTRIBUTE_MODIFIERS, ItemAttributeModifiers.EMPTY)
                .forEach(EquipmentSlot.MAINHAND, (attribute, modifier) -> {
                    if (attribute.equals(Attributes.ATTACK_DAMAGE)) attack[0] += (float) modifier.amount();
                    else if (attribute.equals(Attributes.ATTACK_SPEED)) attack[1] += (float) modifier.amount();
                });
        return attack;
    }

    /** {@code head}, {@code chest}, {@code legs}, {@code feet} as the game's armor type. */
    static ArmorType armorType(String slot) {
        return switch (slot) {
            case "chest" -> ArmorType.CHESTPLATE;
            case "legs" -> ArmorType.LEGGINGS;
            case "feet" -> ArmorType.BOOTS;
            default -> ArmorType.HELMET;
        };
    }

    /** A tool material as the game's. */
    static ToolMaterial material(YogContent.ToolMaterial m) {
        Identifier id = Identifier.tryParse(m.incorrect());
        TagKey<Block> incorrect = id != null ? TagKey.create(Registries.BLOCK, id) : BlockTags.INCORRECT_FOR_WOODEN_TOOL;
        return new ToolMaterial(incorrect, m.durability(), m.speed(), m.attackDamage(), m.enchantability(), repairTag(m.repair()));
    }

    /** A mod's kind of tool: the blocks it mines, by their tag. */
    static TagKey<Block> mineable(String tag) {
        Identifier id = Identifier.tryParse(tag);
        return id != null ? TagKey.create(Registries.BLOCK, id) : BlockTags.MINEABLE_WITH_PICKAXE;
    }

    /** The worn look of armor whose texture is {@code texture}: its equipment asset of the same id. */
    static ResourceKey<EquipmentAsset> asset(String texture) {
        Identifier id = Identifier.tryParse(texture);
        return id != null ? ResourceKey.create(EquipmentAssets.ROOT_ID, id) : EquipmentAssets.IRON;
    }

    /** The tag a repair ingredient names ({@code #tag}), else an empty one of its own id. */
    static TagKey<Item> repairTag(String r) {
        Identifier id = r == null ? null : Identifier.tryParse(r.startsWith(TAG_MARK) ? r.substring(TAG_MARK.length()) : r);
        return TagKey.create(Registries.ITEM, id != null ? id : BuiltInRegistries.ITEM.getKey(Items.AIR));
    }

    /** {@code s} repaired by a single item when the ingredient names one rather than a tag. */
    static Item.Properties repairable(Item.Properties s, String r) {
        if (r == null || r.isEmpty() || r.startsWith(TAG_MARK)) return s;
        Identifier id = Identifier.tryParse(r);
        Item item = id == null ? Items.AIR : BuiltInRegistries.ITEM.getValue(id);
        return item == Items.AIR ? s : s.repairable(item);
    }

    static Component named(String displayName, Component fallback) {
        return displayName == null || displayName.isEmpty() ? fallback : Component.literal(displayName);
    }

    /** The item's tooltip lines: its {@code .desc} translation, else the mod's tooltip. */
    static void hover(Item item, String tip, Consumer<Component> lines) {
        String descKey = item.getDescriptionId() + DESC_SUFFIX;
        Language language = Language.getInstance();
        String resolved = language.has(descKey) ? language.getOrDefault(descKey)
                : (tip != null && !tip.isEmpty() ? tip : null);
        if (resolved != null) {
            for (String line : resolved.split("\n")) lines.accept(Component.literal(line));
        }
    }

    /** A tool or a piece of armor with the mod's name and tooltip. */
    public static final class Named extends Item implements YogTooltipLines {
        private final String displayName, tip;

        Named(Properties s, String displayName, String tip) {
            super(s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void tooltipLines(ItemStack stack, Consumer<Component> lines) {
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.world.inventory.tooltip.TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }
}
