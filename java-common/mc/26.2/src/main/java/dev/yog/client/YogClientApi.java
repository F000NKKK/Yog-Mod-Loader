package dev.yog.client;

import dev.yog.YogLog;
import dev.yog.YogServiceHost;
import dev.yog.actions.YogQueries;
import dev.yog.content.YogAttributes;

import dev.yog.bus.YogTypes;
import com.mojang.blaze3d.platform.InputConstants;
import java.util.Map;
import java.util.StringJoiner;
import java.util.concurrent.ConcurrentHashMap;

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


    /** Flags: 1 = modal, 2 = pause, 4 = no background. */
    @YogServiceHost
    public static void openUI(String uiId, int flags) {
        Minecraft mc = Minecraft.getInstance();
        mc.execute(() -> YogUIScreen.openFlags(uiId, flags));
    }

    @YogServiceHost
    public static int playSound(String id, float volume, float pitch, boolean looping, float from) {
        Identifier rl = Identifier.tryParse(id);
        if (rl == null) return 0;
        SoundInstance s = new SimpleSoundInstance(rl, SoundSource.PLAYERS, volume, pitch,
            SoundInstance.createUnseededRandom(), looping, 0, SoundInstance.Attenuation.NONE,
            0.0, 0.0, 0.0, true);
        return YogSounds.play(s, from);
    }

    @YogServiceHost
    public static void stopSound(int handle) {
        YogSounds.stop(handle);
    }

    @YogServiceHost
    public static YogTypes.Stack heldItem(boolean offHand) {
        LocalPlayer p = Minecraft.getInstance().player;
        if (p == null) return null;
        ItemStack st = p.getItemInHand(offHand ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND);
        if (st.isEmpty()) return null;
        return YogQueries.stack(st);
    }

    /** The key {@code binding} is bound to, as the controls show it; null when there's no such binding. */
    @YogServiceHost
    public static String keyName(String binding) {
        String name = dev.yog.util.YogKeyBindings.nameOf(binding);
        for (net.minecraft.client.KeyMapping k : Minecraft.getInstance().options.keyMappings) {
            if (k.getName().equals(name)) return k.getTranslatedKeyMessage().getString();
        }
        return null;
    }

    @YogServiceHost
    public static boolean keyDown(int key) {
        return InputConstants.isKeyDown(Minecraft.getInstance().getWindow(), key);
    }

    // ── Block tints (client::set_block_tint) ──────────────────────────────────

    private static final Map<String, Map<Long, Integer>> TINTS = new ConcurrentHashMap<>();

    /** Tints the block at x, y, z in {@code dim} (its faces with a tint index); color -1 clears it. */
    @YogServiceHost
    public static void setBlockTint(String dim, int x, int y, int z, int color) {
        long key = net.minecraft.core.BlockPos.asLong(x, y, z);
        Map<Long, Integer> here = TINTS.computeIfAbsent(dim, d -> new ConcurrentHashMap<>());
        if (color < 0) here.remove(key); else here.put(key, color);
        Minecraft mc = Minecraft.getInstance();
        mc.execute(() -> {
            if (mc.level != null && mc.level.dimension().identifier().toString().equals(dim)) {
                mc.levelExtractor.setBlocksDirty(x, y, z, x, y, z);
            }
        });
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
    @YogServiceHost
    public static void bodyTrack(String uuid, boolean on) {
        dev.yog.util.YogBodyWire.track(uuid, on);
    }

    /** yog-body: the same for every entity of a type. */
    @YogServiceHost
    public static void bodyTrackType(String type, boolean on) {
        dev.yog.util.YogBodyWire.trackType(type, on);
    }

    /** yog-body: entity {@code uuid}'s own hitbox on this client, or vanilla's again (width &lt; 0). */
    @YogServiceHost
    public static void setDimensions(String uuid, float width, float height, float eye) {
        try {
            java.util.UUID id = java.util.UUID.fromString(uuid);
            dev.yog.util.YogDimensions.set(true, id, width, height, eye);
            net.minecraft.client.multiplayer.ClientLevel w = net.minecraft.client.Minecraft.getInstance().level;
            if (w == null) return;
            for (net.minecraft.world.entity.Entity e : w.entitiesForRendering()) {
                if (e.getUUID().equals(id)) e.refreshDimensions();
            }
        } catch (IllegalArgumentException e) {
            YogLog.warn("entity dimensions: " + uuid + " isn't a UUID");
        }
    }

    /** An attribute's value on this client's player: its base changed by every modifier. */
    @YogServiceHost
    public static double attributeValue(String attributeId) {
        net.minecraft.world.entity.ai.attributes.AttributeInstance inst =
                YogAttributes.of(net.minecraft.client.Minecraft.getInstance().player, attributeId);
        return inst == null ? Double.NaN : inst.getValue();
    }

    /** Whether this client's player has advancement {@code id} (a book's locked entries ask). */
    @YogServiceHost
    public static boolean advancementDone(String id) {
        var h = net.minecraft.client.Minecraft.getInstance().getConnection();
        if (h == null) return false;
        var map = ((dev.yog.mixin.ClientAdvancementsAccess) h.getAdvancements()).yog$progress();
        for (var e : map.entrySet()) {
            if (e.getKey().id().toString().equals(id)) return e.getValue().isDone();
        }
        return false;
    }

    /** The items of item tag {@code id} ({@code hexcasting:edified_planks}), what a book's recipe shows in turn. */
    @YogServiceHost
    public static java.util.List<String> tagItems(String id) {
        Identifier tag = Identifier.tryParse(id);
        if (tag == null) return java.util.List.of();
        var items = new java.util.ArrayList<String>();
        for (var e : BuiltInRegistries.ITEM.getTagOrEmpty(net.minecraft.tags.TagKey.create(net.minecraft.core.registries.Registries.ITEM, tag))) {
            items.add(BuiltInRegistries.ITEM.getKey(e.value()).toString());
        }
        return items;
    }
}
