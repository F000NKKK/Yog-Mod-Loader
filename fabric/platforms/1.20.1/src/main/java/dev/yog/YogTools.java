package dev.yog;

import dev.yog.data.YogContent;
import java.util.List;
import java.util.Optional;
import net.minecraft.world.World;
import org.jetbrains.annotations.Nullable;
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
            float d = t.attackDamage(), sp = t.attackSpeed();
            return switch (t.kind()) {
                case "pickaxe" -> new Pickaxe(m, (int) d, sp, s, n, tip);
                case "axe" -> new Axe(m, d, sp, s, n, tip);
                case "shovel" -> new Shovel(m, d, sp, s, n, tip);
                case "hoe" -> new Hoe(m, (int) d, sp, s, n, tip);
                case "sword" -> new Sword(m, (int) d, sp, s, n, tip);
                default -> new Digger(d, sp, m, mineable(t.kind()), s, n, tip);
            };
        }
        if (p.armor() != null) {
            YogContent.Armor a = p.armor();
            return new Armor(new YogArmorMaterial(a.material()), armorType(a.slot()), s, p.name(), p.tooltip());
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
        @Override public int getMiningLevel() { return m.levelIndex(); }
        @Override public int getEnchantability() { return m.enchantability(); }
        @Override public Ingredient getRepairIngredient() { return repair(m.repair()); }
    }

    /** An armor material as the game's. */
    record YogArmorMaterial(YogContent.ArmorMaterial m) implements net.minecraft.item.ArmorMaterial {
        @Override public int getDurability(ArmorItem.Type type) {
            return YogContent.armorDurability(new YogContent.Armor(slotOf(type), m));
        }
        @Override public int getProtection(ArmorItem.Type type) {
            return YogContent.armorDefense(new YogContent.Armor(slotOf(type), m));
        }
        @Override public int getEnchantability() { return m.enchantability(); }
        @Override public net.minecraft.sound.SoundEvent getEquipSound() { return YogHost.soundEvent(m.equipSound()); }
        @Override public Ingredient getRepairIngredient() { return repair(m.repair()); }
        /**
         * The texture's name: Fabric 1.20's renderer reads
         * {@code minecraft:textures/models/armor/<name>_layer_1.png} — the
         * id's path, in the minecraft namespace.
         */
        @Override public String getName() {
            int colon = m.texture().indexOf(':');
            return colon < 0 ? m.texture() : m.texture().substring(colon + 1);
        }
        @Override public float getToughness() { return m.toughness(); }
        @Override public float getKnockbackResistance() { return m.knockbackResistance(); }
    }

    private static String slotOf(ArmorItem.Type type) {
        return switch (type) {
            case CHESTPLATE -> "chest";
            case LEGGINGS -> "legs";
            case BOOTS -> "feet";
            default -> "head";
        };
    }

    /** A pickaxe with the mod's name and tooltip. */
    public static final class Pickaxe extends PickaxeItem {
        private final String displayName, tip;

        Pickaxe(ToolMaterial m, int damage, float speed, Settings s, String displayName, String tip) {
            super(m, damage, speed, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, net.minecraft.client.item.TooltipContext context) {
            super.appendTooltip(stack, world, lines, context);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.client.item.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A axe with the mod's name and tooltip. */
    public static final class Axe extends AxeItem {
        private final String displayName, tip;

        Axe(ToolMaterial m, float damage, float speed, Settings s, String displayName, String tip) {
            super(m, damage, speed, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, net.minecraft.client.item.TooltipContext context) {
            super.appendTooltip(stack, world, lines, context);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.client.item.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A shovel with the mod's name and tooltip. */
    public static final class Shovel extends ShovelItem {
        private final String displayName, tip;

        Shovel(ToolMaterial m, float damage, float speed, Settings s, String displayName, String tip) {
            super(m, damage, speed, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, net.minecraft.client.item.TooltipContext context) {
            super.appendTooltip(stack, world, lines, context);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.client.item.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A hoe with the mod's name and tooltip. */
    public static final class Hoe extends HoeItem {
        private final String displayName, tip;

        Hoe(ToolMaterial m, int damage, float speed, Settings s, String displayName, String tip) {
            super(m, damage, speed, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, net.minecraft.client.item.TooltipContext context) {
            super.appendTooltip(stack, world, lines, context);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.client.item.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A sword with the mod's name and tooltip. */
    public static final class Sword extends SwordItem {
        private final String displayName, tip;

        Sword(ToolMaterial m, int damage, float speed, Settings s, String displayName, String tip) {
            super(m, damage, speed, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, net.minecraft.client.item.TooltipContext context) {
            super.appendTooltip(stack, world, lines, context);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.client.item.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A digger with the mod's name and tooltip. */
    public static final class Digger extends MiningToolItem {
        private final String displayName, tip;

        Digger(float damage, float speed, ToolMaterial m, TagKey<Block> mines, Settings s, String displayName, String tip) {
            super(damage, speed, m, mines, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, net.minecraft.client.item.TooltipContext context) {
            super.appendTooltip(stack, world, lines, context);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.client.item.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A armor with the mod's name and tooltip. */
    public static final class Armor extends ArmorItem {
        private final String displayName, tip;

        Armor(net.minecraft.item.ArmorMaterial m, ArmorItem.Type type, Settings s, String displayName, String tip) {
            super(m, type, s);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Text getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendTooltip(ItemStack stack, @Nullable World world, List<Text> lines, net.minecraft.client.item.TooltipContext context) {
            super.appendTooltip(stack, world, lines, context);
            hover(this, tip, lines);
        }

        @Override
        public Optional<net.minecraft.client.item.TooltipData> getTooltipData(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }
}
