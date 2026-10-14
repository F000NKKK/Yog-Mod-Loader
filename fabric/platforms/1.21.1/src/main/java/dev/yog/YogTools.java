package dev.yog;

import dev.yog.data.YogContent;
import java.util.List;
import java.util.Optional;
import net.minecraft.registry.entry.RegistryEntry;
import net.minecraft.block.Block;
import net.minecraft.item.ArmorItem;
import net.minecraft.item.AxeItem;
import net.minecraft.item.HoeItem;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.item.MiningToolItem;
import net.minecraft.item.PickaxeItem;
import net.minecraft.item.ShovelItem;
import net.minecraft.item.SwordItem;
import net.minecraft.item.ToolMaterial;
import net.minecraft.recipe.Ingredient;
import net.minecraft.registry.Registries;
import net.minecraft.registry.RegistryKeys;
import net.minecraft.registry.tag.TagKey;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

/**
 * Tools, weapons and armor of a material (the game's or a mod's), as the
 * runtime describes them ({@link YogContent.Tool}, {@link YogContent.Armor}):
 * the game's own item classes, with the mod's name and tooltip.
 */
public final class YogTools {
    private YogTools() {}

    /** The tool or armor item {@code p} says, or null for neither. */
    public static Item make(Item.Settings s, YogContent.Item p) {
        if (p.tool() != null) {
            YogContent.Tool t = p.tool();
            ToolMaterial m = new YogTier(t.material());
            String n = p.name(), tip = p.tooltip();
            return switch (t.kind()) {
                case "pickaxe" -> new Pickaxe(m, s.attributeModifiers(MiningToolItem.createAttributeModifiers(m, t.attackDamage(), t.attackSpeed())), n, tip);
                case "axe" -> new Axe(m, s.attributeModifiers(MiningToolItem.createAttributeModifiers(m, t.attackDamage(), t.attackSpeed())), n, tip);
                case "shovel" -> new Shovel(m, s.attributeModifiers(MiningToolItem.createAttributeModifiers(m, t.attackDamage(), t.attackSpeed())), n, tip);
                case "hoe" -> new Hoe(m, s.attributeModifiers(MiningToolItem.createAttributeModifiers(m, t.attackDamage(), t.attackSpeed())), n, tip);
                case "sword" -> new Sword(m, s.attributeModifiers(SwordItem.createAttributeModifiers(m, (int) t.attackDamage(), t.attackSpeed())), n, tip);
                default -> new Digger(m, mineable(t.kind()), s.attributeModifiers(MiningToolItem.createAttributeModifiers(m, t.attackDamage(), t.attackSpeed())), n, tip);
            };
        }
        if (p.armor() != null) {
            YogContent.Armor a = p.armor();
            YogContent.ArmorMaterial m = a.material();
            java.util.EnumMap<ArmorItem.Type, Integer> defense = new java.util.EnumMap<>(ArmorItem.Type.class);
            defense.put(ArmorItem.Type.HELMET, m.head());
            defense.put(ArmorItem.Type.CHESTPLATE, m.chest());
            defense.put(ArmorItem.Type.LEGGINGS, m.legs());
            defense.put(ArmorItem.Type.BOOTS, m.feet());
            defense.put(ArmorItem.Type.BODY, m.chest());
            Identifier texture = Identifier.tryParse(m.texture());
            net.minecraft.item.ArmorMaterial material = new net.minecraft.item.ArmorMaterial(defense, m.enchantability(),
                    Registries.SOUND_EVENT.getEntry(YogHost.soundEvent(m.equipSound())), () -> repair(m.repair()),
                    texture == null ? List.of() : List.of(new net.minecraft.item.ArmorMaterial.Layer(texture)),
                    m.toughness(), m.knockbackResistance());
            ArmorItem.Type type = armorType(a.slot());
            return new Armor(RegistryEntry.of(material), type, s.maxDamage(type.getMaxDamage(m.durability())), p.name(), p.tooltip());
        }
        return null;
    }

    /** {@code head}, {@code chest}, {@code legs}, {@code feet} as the game's armor type. */
    static ArmorItem.Type armorType(String slot) {
        return switch (slot) {
            case "chest" -> ArmorItem.Type.CHESTPLATE;
            case "legs" -> ArmorItem.Type.LEGGINGS;
            case "feet" -> ArmorItem.Type.BOOTS;
            default -> ArmorItem.Type.HELMET;
        };
    }

    /** A mod's kind of tool: the blocks it mines, by their tag. */
    static TagKey<Block> mineable(String tag) {
        Identifier id = Identifier.tryParse(tag);
        return TagKey.of(RegistryKeys.BLOCK, id != null ? id : Identifier.tryParse("minecraft:mineable/pickaxe"));
    }

    /** What repairs it: an item id or {@code #tag}; nothing if neither is known. */
    static Ingredient repair(String r) {
        if (r == null || r.isEmpty()) return Ingredient.EMPTY;
        if (r.startsWith("#")) {
            Identifier id = Identifier.tryParse(r.substring(1));
            return id == null ? Ingredient.EMPTY : Ingredient.fromTag(TagKey.of(RegistryKeys.ITEM, id));
        }
        Identifier id = Identifier.tryParse(r);
        Item item = id == null ? Items.AIR : Registries.ITEM.get(id);
        return item == Items.AIR ? Ingredient.EMPTY : Ingredient.ofItems(item);
    }

    static Text named(String displayName, Text fallback) {
        return displayName == null || displayName.isEmpty() ? fallback : Text.literal(displayName);
    }

    /** The item's tooltip lines: its {@code .desc} translation, else the mod's tooltip. */
    static void hover(Item item, String tip, List<Text> lines) {
        String descKey = item.getTranslationKey() + ".desc";
        String resolved = net.minecraft.client.resource.language.I18n.hasTranslation(descKey)
                ? net.minecraft.client.resource.language.I18n.translate(descKey)
                : (tip != null && !tip.isEmpty() ? tip : null);
        if (resolved != null) {
            for (String line : resolved.split("\n")) lines.add(Text.literal(line));
        }
    }

    /** A tool material as the game's. */
    record YogTier(YogContent.ToolMaterial m) implements ToolMaterial {
        @Override public int getDurability() { return m.durability(); }
        @Override public float getMiningSpeedMultiplier() { return m.speed(); }
        @Override public float getAttackDamage() { return m.attackDamage(); }
        @Override public TagKey<Block> getInverseTag() {
            Identifier id = Identifier.tryParse(m.incorrect());
            return TagKey.of(RegistryKeys.BLOCK, id != null ? id : Identifier.of("minecraft", "incorrect_for_wooden_tool"));
        }
        @Override public int getEnchantability() { return m.enchantability(); }
        @Override public Ingredient getRepairIngredient() { return repair(m.repair()); }
    }

    /** A pickaxe with the mod's name and tooltip. */
    public static final class Pickaxe extends PickaxeItem {
        private final String displayName, tip;

        Pickaxe(ToolMaterial m, Settings s, String displayName, String tip) {
            super(m, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, Item.TooltipContext context, List<Text> lines, net.minecraft.item.tooltip.TooltipType type) {
            super.appendTooltip(stack, context, lines, type);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A axe with the mod's name and tooltip. */
    public static final class Axe extends AxeItem {
        private final String displayName, tip;

        Axe(ToolMaterial m, Settings s, String displayName, String tip) {
            super(m, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, Item.TooltipContext context, List<Text> lines, net.minecraft.item.tooltip.TooltipType type) {
            super.appendTooltip(stack, context, lines, type);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A shovel with the mod's name and tooltip. */
    public static final class Shovel extends ShovelItem {
        private final String displayName, tip;

        Shovel(ToolMaterial m, Settings s, String displayName, String tip) {
            super(m, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, Item.TooltipContext context, List<Text> lines, net.minecraft.item.tooltip.TooltipType type) {
            super.appendTooltip(stack, context, lines, type);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A hoe with the mod's name and tooltip. */
    public static final class Hoe extends HoeItem {
        private final String displayName, tip;

        Hoe(ToolMaterial m, Settings s, String displayName, String tip) {
            super(m, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, Item.TooltipContext context, List<Text> lines, net.minecraft.item.tooltip.TooltipType type) {
            super.appendTooltip(stack, context, lines, type);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A sword with the mod's name and tooltip. */
    public static final class Sword extends SwordItem {
        private final String displayName, tip;

        Sword(ToolMaterial m, Settings s, String displayName, String tip) {
            super(m, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, Item.TooltipContext context, List<Text> lines, net.minecraft.item.tooltip.TooltipType type) {
            super.appendTooltip(stack, context, lines, type);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A digger with the mod's name and tooltip. */
    public static final class Digger extends MiningToolItem {
        private final String displayName, tip;

        Digger(ToolMaterial m, TagKey<Block> mines, Settings s, String displayName, String tip) {
            super(m, mines, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, Item.TooltipContext context, List<Text> lines, net.minecraft.item.tooltip.TooltipType type) {
            super.appendTooltip(stack, context, lines, type);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A armor with the mod's name and tooltip. */
    public static final class Armor extends ArmorItem {
        private final String displayName, tip;

        Armor(RegistryEntry<net.minecraft.item.ArmorMaterial> m, ArmorItem.Type type, Settings s, String displayName, String tip) {
            super(m, type, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, Item.TooltipContext context, List<Text> lines, net.minecraft.item.tooltip.TooltipType type) {
            super.appendTooltip(stack, context, lines, type);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.item.tooltip.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }
}
