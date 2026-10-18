package dev.yog;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.color.block.BlockColor;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.renderer.entity.EntityRendererProvider;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
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

    /** Where the mods' menu buttons go: the title screen's top-left corner, clear of vanilla's. */
    private static final int MENU_X = 4, MENU_Y = 4, MENU_WIDTH = 100, MENU_HEIGHT = 20, MENU_STEP = 24;
    /** On a loader's mod list the buttons stand at the top right, this far in from the edge. */
    private static final int MOD_LIST_INSET = 110, MOD_LIST_Y = 10;
    /** The loaders' mod list screens, by class name (the mods' menu entries go there too). */
    private static final List<String> MOD_LISTS = List.of(
            "net.neoforged.neoforge.client.gui.ModListScreen", "net.minecraftforge.client.gui.ModListScreen");

    /** The end of a client tick: key presses, then the mods. */
    public static void tick() {
        dev.yog.util.YogKeyBindings.tick(NativeBridge::nativeOnKeyBinding);
        NativeBridge.nativeOnClientTick();
    }

    /** The HUD drawn this frame: the mods draw in it. */
    public static void hud(GuiGraphics ctx, float partialTick) {
        NativeBridge.nativeGlInit();
        Minecraft mc = Minecraft.getInstance();
        Vec3 eye = mc.player != null ? mc.player.getEyePosition() : Vec3.ZERO;
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeOnHudRender(partialTick,
                mc.getWindow().getGuiScaledWidth(), mc.getWindow().getGuiScaledHeight(), (float) mc.getWindow().getGuiScale(),
                (float) eye.x, (float) eye.y, (float) eye.z);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState();
    }

    /** The level drawn: the mods draw in it, with its {@code projection} and {@code modelView}, the camera at {@code camera}. */
    public static void world(Matrix4f projection, Matrix4f modelView, Vec3 camera, float partialTick) {
        NativeBridge.nativeGlInit();
        Minecraft mc = Minecraft.getInstance();
        float[] vp = new float[16];
        new Matrix4f(projection).mul(modelView).get(vp);
        Vec3 eye = mc.player != null ? mc.player.getEyePosition() : camera;
        NativeBridge.nativeOnWorldRender(partialTick,
                mc.getWindow().getGuiScaledWidth(), mc.getWindow().getGuiScaledHeight(), (float) mc.getWindow().getGuiScale(),
                vp, (float) camera.x, (float) camera.y, (float) camera.z, (float) eye.x, (float) eye.y, (float) eye.z);
        NativeDraw.syncGlState();
    }

    public static void screenOpened(Screen screen) {
        if (screen instanceof YogUIScreen) return;
        NativeBridge.nativeOnScreenOpen(screen.getClass().getSimpleName());
    }

    public static void screenClosed(Screen screen) {
        if (screen instanceof YogUIScreen) return;
        NativeBridge.nativeOnScreenClose(screen.getClass().getSimpleName());
    }

    /** The mods' menu entries as buttons, on the title screen and a loader's mod list (none on others). */
    public static List<Button> menuButtons(Screen screen) {
        List<Button> out = new ArrayList<>();
        boolean modList = MOD_LISTS.contains(screen.getClass().getName());
        if (!modList && !(screen instanceof net.minecraft.client.gui.screens.TitleScreen)) return out;
        String raw = NativeBridge.nativeMenuEntries();
        if (raw == null || raw.isEmpty()) return out;
        int x = modList ? screen.width - MOD_LIST_INSET : MENU_X;
        int y = modList ? MOD_LIST_Y : MENU_Y;
        for (String line : raw.split("\n")) {
            String[] parts = line.split("\t", 2);
            if (parts.length < 2) continue;
            String uiId = parts[1];
            out.add(Button.builder(Component.translatable(parts[0]), b -> YogUIScreen.open(uiId, false, false))
                    .pos(x, y).size(MENU_WIDTH, MENU_HEIGHT).build());
            y += MENU_STEP;
        }
        return out;
    }

    /** The mods' key bindings (ABI 3.10): each a game key under its mod's category; their presses go to the mods each tick. */
    public static List<KeyMapping> keyMappings() {
        NativeBridge.ensureLoaded();
        List<KeyMapping> out = new ArrayList<>();
        for (dev.yog.util.YogKeyBindings.Binding b : dev.yog.util.YogKeyBindings.declared(NativeBridge.nativeKeyBindings())) {
            KeyMapping key = new KeyMapping(b.name(), com.mojang.blaze3d.platform.InputConstants.Type.KEYSYM, b.defaultKey(), b.categoryKey());
            dev.yog.util.YogKeyBindings.track(b.id(), key::consumeClick);
            out.add(key);
        }
        return out;
    }

    /** The mods' blocks, which take the tints mods set per position ({@link #tint}). */
    public static List<Block> tintedBlocks() {
        NativeBridge.ensureLoaded();
        List<Block> out = new ArrayList<>();
        for (dev.yog.data.YogContent.Block b : dev.yog.data.YogContent.cached(NativeBridge::nativeContent).blocks) {
            ResourceLocation id = ResourceLocation.tryParse(b.id());
            if (id != null && BuiltInRegistries.BLOCK.containsKey(id)) out.add(BuiltInRegistries.BLOCK.get(id));
        }
        return out;
    }

    /** The tint a mod set at a block's position ({@code client::set_block_tint}). */
    public static final BlockColor tint = (state, level, pos, tintIndex) -> YogClientApi.tint(pos);

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

    /** A world left: its entity data is forgotten. */
    public static void disconnected() {
        NativeBridge.nativeEntityDataClientReset();
    }
}
