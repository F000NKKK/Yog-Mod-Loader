package dev.yog;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.Map;
import java.util.StringJoiner;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.resources.sounds.SimpleSoundInstance;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.ItemStack;

/**
 * Client-side primitives for Rust mods (ABI minor 1): opening a screen with
 * flags, sounds only the local player hears, held items, key state. Called
 * from the runtime (`client_api.rs`); every method is a no-op on a
 * dedicated server (the client classes are missing there).
 */
public final class YogClientApi {
    private YogClientApi() {}

    private static final Map<Integer, SoundInstance> SOUNDS = new ConcurrentHashMap<>();
    private static final AtomicInteger NEXT = new AtomicInteger(1);

    /** Flags: 1 = modal, 2 = pause, 4 = no background. */
    public static void openUI(String uiId, int flags) {
        try {
            Minecraft mc = Minecraft.getInstance();
            mc.execute(() -> YogUIScreen.openFlags(uiId, flags));
        } catch (Throwable t) {
            // dedicated server
        }
    }

    public static int playSound(String id, float volume, float pitch, boolean looping) {
        try {
            Minecraft mc = Minecraft.getInstance();
            Identifier rl = Identifier.tryParse(id);
            if (rl == null) return 0;
            SoundInstance s = new SimpleSoundInstance(rl, SoundSource.PLAYERS, volume, pitch,
                SoundInstance.createUnseededRandom(), looping, 0, SoundInstance.Attenuation.NONE,
                0.0, 0.0, 0.0, true);
            int h = NEXT.getAndIncrement();
            if (looping) SOUNDS.put(h, s);
            mc.execute(() -> mc.getSoundManager().play(s));
            return h;
        } catch (Throwable t) {
            return 0;
        }
    }

    public static void stopSound(int handle) {
        SoundInstance s = SOUNDS.remove(handle);
        if (s == null) return;
        try {
            Minecraft mc = Minecraft.getInstance();
            mc.execute(() -> mc.getSoundManager().stop(s));
        } catch (Throwable t) {
            // dedicated server
        }
    }

    public static String heldItem(boolean offHand) {
        try {
            LocalPlayer p = Minecraft.getInstance().player;
            if (p == null) return null;
            ItemStack st = p.getItemInHand(offHand ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND);
            if (st.isEmpty()) return null;
            return BuiltInRegistries.ITEM.getKey(st.getItem()).toString();
        } catch (Throwable t) {
            return null;
        }
    }

    public static boolean keyDown(int key) {
        try {
            return InputConstants.isKeyDown(Minecraft.getInstance().getWindow(), key);
        } catch (Throwable t) {
            return false;
        }
    }

    // ── Block tints (client::set_block_tint) ──────────────────────────────────

    private static final Map<String, Map<Long, Integer>> TINTS = new ConcurrentHashMap<>();

    /** Tints the block at x, y, z in {@code dim} (its faces with a tint index); color -1 clears it. */
    public static void setBlockTint(String dim, int x, int y, int z, int color) {
        long key = net.minecraft.core.BlockPos.asLong(x, y, z);
        Map<Long, Integer> here = TINTS.computeIfAbsent(dim, d -> new ConcurrentHashMap<>());
        if (color < 0) here.remove(key); else here.put(key, color);
        try {
            Minecraft mc = Minecraft.getInstance();
            mc.execute(() -> {
                if (mc.level != null && mc.level.dimension().identifier().toString().equals(dim)) {
                    mc.levelExtractor.setBlocksDirty(x, y, z, x, y, z);
                }
            });
        } catch (Throwable t) {
            // dedicated server
        }
    }

    /** The tint at {@code pos} in the client's world, white if none. */
    public static int tint(net.minecraft.core.BlockPos pos) {
        Minecraft mc = Minecraft.getInstance();
        if (mc.level == null || pos == null) return 0xFFFFFF;
        Map<Long, Integer> here = TINTS.get(mc.level.dimension().identifier().toString());
        Integer c = here == null ? null : here.get(pos.asLong());
        return c == null ? 0xFFFFFF : c;
    }

    /** The tint a mod set per position, on block faces with a tint index (opaque white where none). */
    public static final class Tint implements net.minecraft.client.color.block.BlockTintSource {
        @Override
        public int color(net.minecraft.world.level.block.state.BlockState state) {
            return net.minecraft.util.ARGB.opaque(0xFFFFFF);
        }

        @Override
        public int colorInWorld(net.minecraft.world.level.block.state.BlockState state,
                                net.minecraft.client.renderer.block.BlockAndTintGetter level, net.minecraft.core.BlockPos pos) {
            return net.minecraft.util.ARGB.opaque(tint(pos));
        }
    }

    /** yog-body: ask the mods about entity {@code uuid}'s body each frame, or stop. */
    public static void bodyTrack(String uuid, boolean on) {
        dev.yog.util.YogBodyWire.track(uuid, on);
    }

    /** yog-body: the same for every entity of a type. */
    public static void bodyTrackType(String type, boolean on) {
        dev.yog.util.YogBodyWire.trackType(type, on);
    }

    /** yog-body: entity {@code uuid}'s own hitbox on this client, or vanilla's again (width &lt; 0). */
    public static void setDimensions(String uuid, float width, float height, float eye) {
        try {
            java.util.UUID id = java.util.UUID.fromString(uuid);
            dev.yog.util.YogDimensions.set(true, id, width, height, eye);
            net.minecraft.client.multiplayer.ClientLevel w = net.minecraft.client.Minecraft.getInstance().level;
            if (w == null) return;
            for (net.minecraft.world.entity.Entity e : w.entitiesForRendering()) {
                if (e.getUUID().equals(id)) e.refreshDimensions();
            }
        } catch (IllegalArgumentException ignored) {
        }
    }

    /** An attribute's value on this client's player: its base changed by every modifier. */
    public static double attributeValue(String attributeId) {
        net.minecraft.world.entity.LivingEntity le = net.minecraft.client.Minecraft.getInstance().player;
        if (le == null) return Double.NaN;
        net.minecraft.resources.Identifier id = net.minecraft.resources.Identifier.tryParse(attributeId);
        if (id == null || !net.minecraft.core.registries.BuiltInRegistries.ATTRIBUTE.containsKey(id)) return Double.NaN;
        net.minecraft.world.entity.ai.attributes.Attribute attr = net.minecraft.core.registries.BuiltInRegistries.ATTRIBUTE.getValue(id);
        if (attr == null) return Double.NaN;
        net.minecraft.world.entity.ai.attributes.AttributeInstance inst = le.getAttribute(net.minecraft.core.registries.BuiltInRegistries.ATTRIBUTE.wrapAsHolder(attr));
        return inst == null ? Double.NaN : inst.getValue();
    }

    /** Whether this client's player has advancement {@code id} (a book's locked entries ask). */
    public static boolean advancementDone(String id) {
        var h = net.minecraft.client.Minecraft.getInstance().getConnection();
        if (h == null) return false;
        var map = ((dev.yog.mixin.ClientAdvancementsAccess) h.getAdvancements()).yog$progress();
        for (var e : map.entrySet()) {
            if (e.getKey().id().toString().equals(id)) return e.getValue().isDone();
        }
        return false;
    }

    /** The items of item tag {@code id} ({@code hexcasting:edified_planks}), {@link YogNames.Sep#FIELD}-separated: what a book's recipe shows in turn. */
    public static String tagItems(String id) {
        Identifier tag = Identifier.tryParse(id);
        if (tag == null) return "";
        var items = new java.util.StringJoiner(YogNames.Sep.FIELD);
        for (var e : BuiltInRegistries.ITEM.getTagOrEmpty(net.minecraft.tags.TagKey.create(net.minecraft.core.registries.Registries.ITEM, tag))) {
            items.add(BuiltInRegistries.ITEM.getKey(e.value()).toString());
        }
        return items.toString();
    }
}
