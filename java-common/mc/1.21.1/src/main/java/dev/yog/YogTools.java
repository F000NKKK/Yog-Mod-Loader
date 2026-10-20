package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.world.item.component.ItemAttributeModifiers;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.core.component.DataComponents;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.tags.TagKey;
import net.minecraft.world.inventory.tooltip.TooltipComponent;
import net.minecraft.world.item.ArmorItem;
import net.minecraft.world.item.AxeItem;
import net.minecraft.world.item.DiggerItem;
import net.minecraft.world.item.HoeItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.PickaxeItem;
import net.minecraft.world.item.ShovelItem;
import net.minecraft.world.item.SwordItem;
import net.minecraft.world.item.Tier;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.item.crafting.Ingredient;
import net.minecraft.world.level.block.Block;

/**
 * Tools, weapons and armor of a material (the game's or a mod's), as the
 * runtime describes them ({@link YogContent.Tool}, {@link YogContent.Armor}):
 * the game's own item classes, with the mod's name and tooltip.
 */
public final class YogTools {
    private YogTools() {}

    /** The tool or armor item {@code p} says, or null for neither. */
    public static Item make(Item.Properties props, YogContent.Item p) {
        if (p.tool() != null) {
            YogContent.Tool t = p.tool().resolved(YogTools::gameAttack);
            Tier tier = new YogTier(t.material());
            String n = p.name(), tip = p.tooltip();
            return switch (t.kind()) {
                case "pickaxe" -> new Pickaxe(tier, props.attributes(DiggerItem.createAttributes(tier, t.attackDamage(), t.attackSpeed())), n, tip);
                case "axe" -> new Axe(tier, props.attributes(DiggerItem.createAttributes(tier, t.attackDamage(), t.attackSpeed())), n, tip);
                case "shovel" -> new Shovel(tier, props.attributes(DiggerItem.createAttributes(tier, t.attackDamage(), t.attackSpeed())), n, tip);
                case "hoe" -> new Hoe(tier, props.attributes(DiggerItem.createAttributes(tier, t.attackDamage(), t.attackSpeed())), n, tip);
                case "sword" -> new Sword(tier, props.attributes(SwordItem.createAttributes(tier, (int) t.attackDamage(), t.attackSpeed())), n, tip);
                default -> new Digger(tier, mineable(t.kind()), props.attributes(DiggerItem.createAttributes(tier, t.attackDamage(), t.attackSpeed())), n, tip);
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
            ResourceLocation texture = ResourceLocation.tryParse(m.texture());
            net.minecraft.world.item.ArmorMaterial material = new net.minecraft.world.item.ArmorMaterial(defense, m.enchantability(),
                    BuiltInRegistries.SOUND_EVENT.wrapAsHolder(YogContentRegistry.soundEvent(m.equipSound())), () -> repair(m.repair()),
                    texture == null ? List.of() : List.of(new net.minecraft.world.item.ArmorMaterial.Layer(texture)),
                    m.toughness(), m.knockbackResistance());
            ArmorItem.Type type = armorType(a.slot());
            return new Armor(Holder.direct(material), type, props.durability(type.getDurability(m.durability())), p.name(), p.tooltip());
        }
        return null;
    }

    /** Game item {@code id}'s main-hand attack damage and speed modifiers; null for no such item. */
    static float[] gameAttack(String id) {
        ResourceLocation key = ResourceLocation.tryParse(id);
        Item item = key == null ? Items.AIR : BuiltInRegistries.ITEM.get(key);
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
        ResourceLocation rl = ResourceLocation.tryParse(tag);
        return TagKey.create(Registries.BLOCK, rl != null ? rl : ResourceLocation.tryParse(YogNames.Game.MINEABLE + YogNames.ToolKind.PICKAXE));
    }

    /** What repairs it: an item id or {@code #tag}; nothing if neither is known. */
    static Ingredient repair(String r) {
        if (r == null || r.isEmpty()) return Ingredient.EMPTY;
        if (r.startsWith("#")) {
            ResourceLocation rl = ResourceLocation.tryParse(r.substring(1));
            return rl == null ? Ingredient.EMPTY : Ingredient.of(TagKey.create(Registries.ITEM, rl));
        }
        ResourceLocation rl = ResourceLocation.tryParse(r);
        Item item = rl == null ? Items.AIR : BuiltInRegistries.ITEM.get(rl);
        return item == Items.AIR ? Ingredient.EMPTY : Ingredient.of(item);
    }

    static Component named(String displayName, Component fallback) {
        return displayName == null || displayName.isEmpty() ? fallback : Component.literal(displayName);
    }

    /** The item's tooltip lines: its {@code .desc} translation, else the mod's tooltip. */
    static void hover(Item item, String tip, List<Component> lines) {
        String descKey = item.getDescriptionId() + ".desc";
        String resolved = net.minecraft.client.resources.language.I18n.exists(descKey)
                ? net.minecraft.client.resources.language.I18n.get(descKey)
                : (tip != null && !tip.isEmpty() ? tip : null);
        if (resolved != null) {
            for (String line : resolved.split("\n")) lines.add(Component.literal(line));
        }
    }

    /** A tool material as the game's tier. */
    record YogTier(YogContent.ToolMaterial m) implements Tier {
        @Override public int getUses() { return m.durability(); }
        @Override public float getSpeed() { return m.speed(); }
        @Override public float getAttackDamageBonus() { return m.attackDamage(); }
        @Override public TagKey<Block> getIncorrectBlocksForDrops() {
            ResourceLocation rl = ResourceLocation.tryParse(m.incorrect());
            return TagKey.create(Registries.BLOCK, rl != null ? rl : ResourceLocation.tryParse("minecraft:incorrect_for_wooden_tool"));
        }
        @Override public int getEnchantmentValue() { return m.enchantability(); }
        @Override public Ingredient getRepairIngredient() { return repair(m.repair()); }
    }

    /** A pickaxe with the mod's name and tooltip. */
    public static final class Pickaxe extends PickaxeItem {
        private final String displayName, tip;

        Pickaxe(Tier tier, Properties p, String displayName, String tip) {
            super(tier, p);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
            super.appendHoverText(stack, context, lines, flag);
            hover(this, tip, lines);
        }

        @Override
        public Optional<TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A axe with the mod's name and tooltip. */
    public static final class Axe extends AxeItem {
        private final String displayName, tip;

        Axe(Tier tier, Properties p, String displayName, String tip) {
            super(tier, p);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
            super.appendHoverText(stack, context, lines, flag);
            hover(this, tip, lines);
        }

        @Override
        public Optional<TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A shovel with the mod's name and tooltip. */
    public static final class Shovel extends ShovelItem {
        private final String displayName, tip;

        Shovel(Tier tier, Properties p, String displayName, String tip) {
            super(tier, p);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
            super.appendHoverText(stack, context, lines, flag);
            hover(this, tip, lines);
        }

        @Override
        public Optional<TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A hoe with the mod's name and tooltip. */
    public static final class Hoe extends HoeItem {
        private final String displayName, tip;

        Hoe(Tier tier, Properties p, String displayName, String tip) {
            super(tier, p);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
            super.appendHoverText(stack, context, lines, flag);
            hover(this, tip, lines);
        }

        @Override
        public Optional<TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A sword with the mod's name and tooltip. */
    public static final class Sword extends SwordItem {
        private final String displayName, tip;

        Sword(Tier tier, Properties p, String displayName, String tip) {
            super(tier, p);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
            super.appendHoverText(stack, context, lines, flag);
            hover(this, tip, lines);
        }

        @Override
        public Optional<TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A digger with the mod's name and tooltip. */
    public static final class Digger extends DiggerItem {
        private final String displayName, tip;

        Digger(Tier tier, TagKey<Block> mines, Properties p, String displayName, String tip) {
            super(tier, mines, p);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
            super.appendHoverText(stack, context, lines, flag);
            hover(this, tip, lines);
        }

        @Override
        public Optional<TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }

    /** A armor with the mod's name and tooltip. */
    public static final class Armor extends ArmorItem {
        private final String displayName, tip;

        Armor(Holder<net.minecraft.world.item.ArmorMaterial> m, ArmorItem.Type type, Properties p, String displayName, String tip) {
            super(m, type, p);
            this.displayName = displayName;
            this.tip = tip;
        }

        @Override
        public Component getName(ItemStack stack) {
            return named(displayName, super.getName(stack));
        }

        @Override
        public void appendHoverText(ItemStack stack, Item.TooltipContext context, List<Component> lines, TooltipFlag flag) {
            super.appendHoverText(stack, context, lines, flag);
            hover(this, tip, lines);
        }

        @Override
        public Optional<TooltipComponent> getTooltipImage(ItemStack stack) {
            return YogTooltipData.of(stack);
        }
    }
}
