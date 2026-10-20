package dev.yog;

import dev.yog.bus.YogEvents;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Function;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.renderer.entity.EntityRendererProvider;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;

/**
 * What the client's events mean to the mods, the same on every loader: a
 * loader's client hooks its own events up to these.
 */
public final class YogClientEvents {
    private YogClientEvents() {}

    /** The projection the level was last drawn with (bobbing and all), set by {@code LevelProjectionMixin}. */
    public static final Matrix4f levelProjection = new Matrix4f();

    /** Where the mods' menu buttons go on the title screen: its top-left corner, clear of vanilla's. */
    private static final int MENU_X = 4, MENU_Y = 4, MENU_WIDTH = 100, MENU_HEIGHT = 20, MENU_STEP = 24;

    /** The end of a client tick: key presses, then the mods. */
    public static void tick() {
        dev.yog.util.YogKeyBindings.tick();
        new YogEvents.Client.Game.Tick().emit();
    }

    /** The HUD being extracted for this frame: the mods' 2-D draws go into it, their raw GL onto the main target. */
    public static void hud(GuiGraphicsExtractor ctx, float partialTick) {
        NativeBridge.nativeGlInit();
        Minecraft mc = Minecraft.getInstance();
        Vec3 eye = mc.player != null ? mc.player.getEyePosition() : Vec3.ZERO;
        NativeDraw.hudDrawContext = ctx;
        boolean gl = YogGl.begin();
        NativeBridge.nativeOnHudRender(partialTick,
                mc.getWindow().getGuiScaledWidth(), mc.getWindow().getGuiScaledHeight(), (float) mc.getWindow().getGuiScale(),
                (float) eye.x, (float) eye.y, (float) eye.z);
        if (gl) YogGl.end();
        NativeDraw.hudDrawContext = null;
    }

    /**
     * The level drawn (its main pass): the mods draw in it, with the camera's
     * rotation {@code viewRotation}, the level's projection and the camera at
     * {@code camera} (geometry is relative to it).
     */
    public static void world(Matrix4f viewRotation, Vec3 camera, float partialTick) {
        NativeBridge.nativeGlInit();
        Minecraft mc = Minecraft.getInstance();
        float[] vp = new float[16];
        new Matrix4f(levelProjection).mul(viewRotation).get(vp);
        Vec3 eye = mc.player != null ? mc.player.getEyePosition() : camera;
        if (!YogGl.begin()) return;
        NativeBridge.nativeOnWorldRender(partialTick,
                mc.getWindow().getGuiScaledWidth(), mc.getWindow().getGuiScaledHeight(), (float) mc.getWindow().getGuiScale(),
                vp, (float) camera.x, (float) camera.y, (float) camera.z, (float) eye.x, (float) eye.y, (float) eye.z);
        YogGl.end();
    }

    public static void screenOpened(Screen screen) {
        if (screen instanceof YogUIScreen) return;
        new YogEvents.Client.Ui.Open(screen.getClass().getSimpleName()).emit();
    }

    public static void screenClosed(Screen screen) {
        if (screen instanceof YogUIScreen) return;
        new YogEvents.Client.Ui.Close(screen.getClass().getSimpleName()).emit();
    }

    /** The mods' menu entries as buttons for the title screen (none on other screens). */
    public static List<Button> menuButtons(Screen screen) {
        List<Button> out = new ArrayList<>();
        if (!(screen instanceof net.minecraft.client.gui.screens.TitleScreen)) return out;
        String raw = NativeBridge.nativeMenuEntries();
        if (raw == null || raw.isEmpty()) return out;
        int y = MENU_Y;
        for (String line : raw.split("\n")) {
            String[] parts = line.split("\t", 2);
            if (parts.length < 2) continue;
            String uiId = parts[1];
            out.add(Button.builder(Component.translatable(parts[0]), b -> YogUIScreen.open(uiId, false, false))
                    .pos(MENU_X, y).size(MENU_WIDTH, MENU_HEIGHT).build());
            y += MENU_STEP;
        }
        return out;
    }

    /**
     * The mods' key bindings (ABI 3.10): each a game key under its mod's
     * category ({@code category} makes it, the loader's way); their presses
     * go to the mods each tick.
     */
    public static List<KeyMapping> keyMappings(Function<Identifier, KeyMapping.Category> category) {
        NativeBridge.ensureLoaded();
        java.util.Map<String, KeyMapping.Category> categories = new java.util.HashMap<>();
        List<KeyMapping> out = new ArrayList<>();
        for (dev.yog.util.YogKeyBindings.Binding b : dev.yog.util.YogKeyBindings.declared(NativeBridge.nativeKeyBindings())) {
            KeyMapping.Category c = categories.computeIfAbsent(b.namespace(),
                    ns -> category.apply(Identifier.fromNamespaceAndPath(ns, dev.yog.util.YogKeyBindings.CATEGORY_PATH)));
            KeyMapping key = new KeyMapping(b.name(), com.mojang.blaze3d.platform.InputConstants.Type.KEYSYM, b.defaultKey(), c);
            dev.yog.util.YogKeyBindings.track(b.id(), key::consumeClick);
            out.add(key);
        }
        return out;
    }

    /** The mods' blocks, which take the tints mods set per position ({@link YogClientApi.Tint}). */
    public static List<Block> tintedBlocks() {
        NativeBridge.ensureLoaded();
        List<Block> out = new ArrayList<>();
        for (dev.yog.data.YogContent.Block b : dev.yog.data.YogContent.cached(NativeBridge::nativeContent).blocks) {
            Identifier id = Identifier.tryParse(b.id());
            if (id != null && BuiltInRegistries.BLOCK.containsKey(id)) out.add(BuiltInRegistries.BLOCK.getValue(id));
        }
        return out;
    }

    /** How a loader registers an entity type's renderer. */
    public interface Renderers {
        void register(EntityType<Entity> type, EntityRendererProvider<Entity> provider);
    }

    /** The mods' entities: drawn by what the mods give (yog-body). */
    @SuppressWarnings("unchecked")
    public static void renderers(Renderers out) {
        for (EntityType<?> type : YogEntityTypes.types()) {
            out.register((EntityType<Entity>) type, ctx -> new YogEntityRenderer(ctx, YogEntityTypes.def(type)));
        }
    }

    /** A stack's own tooltip lines, from the mods. */
    public static void tooltip(net.minecraft.world.item.ItemStack stack, Consumer<Component> lines) {
        if (stack.getItem() instanceof YogTooltipLines own) own.tooltipLines(stack, lines);
    }

    /** A world left: its entity data is forgotten. */
    public static void disconnected() {
        NativeBridge.nativeEntityDataClientReset();
    }
}
