package dev.yog;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.resources.sounds.SimpleSoundInstance;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
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
            ResourceLocation rl = ResourceLocation.tryParse(id);
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

    @SuppressWarnings("deprecation") // BuiltInRegistries is the vanilla registry; Forge only nudges towards its wrappers
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
            return InputConstants.isKeyDown(Minecraft.getInstance().getWindow().getWindow(), key);
        } catch (Throwable t) {
            return false;
        }
    }
}
