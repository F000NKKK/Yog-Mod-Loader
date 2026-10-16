package dev.yog;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.sound.PositionedSoundInstance;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.client.util.InputUtil;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.Hand;
import net.minecraft.util.Identifier;

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
            MinecraftClient mc = MinecraftClient.getInstance();
            mc.execute(() -> YogUIScreen.openFlags(uiId, flags));
        } catch (Throwable t) {
            // dedicated server
        }
    }

    public static int playSound(String id, float volume, float pitch, boolean looping) {
        try {
            MinecraftClient mc = MinecraftClient.getInstance();
            Identifier rl = Identifier.tryParse(id);
            if (rl == null) return 0;
            SoundInstance s = new PositionedSoundInstance(rl, SoundCategory.PLAYERS, volume, pitch,
                SoundInstance.createRandom(), looping, 0, SoundInstance.AttenuationType.NONE,
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
            MinecraftClient mc = MinecraftClient.getInstance();
            mc.execute(() -> mc.getSoundManager().stop(s));
        } catch (Throwable t) {
            // dedicated server
        }
    }

    public static String heldItem(boolean offHand) {
        try {
            ClientPlayerEntity p = MinecraftClient.getInstance().player;
            if (p == null) return null;
            ItemStack st = p.getStackInHand(offHand ? Hand.OFF_HAND : Hand.MAIN_HAND);
            if (st.isEmpty()) return null;
            return Registries.ITEM.getId(st.getItem()).toString();
        } catch (Throwable t) {
            return null;
        }
    }

    public static boolean keyDown(int key) {
        try {
            return InputUtil.isKeyPressed(MinecraftClient.getInstance().getWindow().getHandle(), key);
        } catch (Throwable t) {
            return false;
        }
    }

    // ── Block tints (client::set_block_tint) ──────────────────────────────────

    private static final Map<String, Map<Long, Integer>> TINTS = new ConcurrentHashMap<>();

    /** Tints the block at x, y, z in {@code dim} (its faces with a tint index); color -1 clears it. */
    public static void setBlockTint(String dim, int x, int y, int z, int color) {
        long key = net.minecraft.util.math.BlockPos.asLong(x, y, z);
        Map<Long, Integer> here = TINTS.computeIfAbsent(dim, d -> new ConcurrentHashMap<>());
        if (color < 0) here.remove(key); else here.put(key, color);
        try {
            MinecraftClient mc = MinecraftClient.getInstance();
            mc.execute(() -> {
                if (mc.world != null && mc.world.getRegistryKey().getValue().toString().equals(dim)) {
                    mc.worldRenderer.scheduleBlockRenders(x, y, z, x, y, z);
                }
            });
        } catch (Throwable t) {
            // dedicated server
        }
    }

    /** The tint at {@code pos} in the client's world, white if none. */
    public static int tint(net.minecraft.util.math.BlockPos pos) {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc.world == null || pos == null) return 0xFFFFFF;
        Map<Long, Integer> here = TINTS.get(mc.world.getRegistryKey().getValue().toString());
        Integer c = here == null ? null : here.get(pos.asLong());
        return c == null ? 0xFFFFFF : c;
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
            net.minecraft.client.world.ClientWorld w = net.minecraft.client.MinecraftClient.getInstance().world;
            if (w == null) return;
            for (net.minecraft.entity.Entity e : w.getEntities()) {
                if (e.getUuid().equals(id)) e.calculateDimensions();
            }
        } catch (IllegalArgumentException ignored) {
        }
    }

    /** An attribute's value on this client's player: its base changed by every modifier. */
    public static double attributeValue(String attributeId) {
        net.minecraft.entity.LivingEntity le = net.minecraft.client.MinecraftClient.getInstance().player;
        if (le == null) return Double.NaN;
        net.minecraft.util.Identifier id = net.minecraft.util.Identifier.tryParse(attributeId);
        if (id == null || !net.minecraft.registry.Registries.ATTRIBUTE.containsId(id)) return Double.NaN;
        net.minecraft.entity.attribute.EntityAttribute attr = net.minecraft.registry.Registries.ATTRIBUTE.get(id);
        if (attr == null) return Double.NaN;
        net.minecraft.entity.attribute.EntityAttributeInstance inst = le.getAttributeInstance(net.minecraft.registry.Registries.ATTRIBUTE.getEntry(attr));
        return inst == null ? Double.NaN : inst.getValue();
    }

    /** Whether this client's player has advancement {@code id} (a book's locked entries ask). */
    public static boolean advancementDone(String id) {
        var h = net.minecraft.client.MinecraftClient.getInstance().getNetworkHandler();
        if (h == null) return false;
        var map = ((dev.yog.mixin.ClientAdvancementsAccess) h.getAdvancementHandler()).yog$progress();
        for (var e : map.entrySet()) {
            if (e.getKey().id().toString().equals(id)) return e.getValue().isDone();
        }
        return false;
    }
}
