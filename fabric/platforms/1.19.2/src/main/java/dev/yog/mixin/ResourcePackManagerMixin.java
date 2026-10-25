package dev.yog.mixin;

import dev.yog.YogLog;

import java.util.HashSet;
import java.util.Set;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraft.server.packs.repository.ServerPacksSource;
import dev.yog.content.YogPackProvider;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Adds the Yog pack provider to every resource/data pack manager, so the assets
 * and data bundled in .yog mods are served to the client and the server.
 */
@Mixin(PackRepository.class)
public class ResourcePackManagerMixin {
    @Shadow
    @Final
    @Mutable
    private Set<RepositorySource> sources;

    @Inject(method = "<init>", at = @At("TAIL"))
    private void yog$addProvider(RepositorySource[] providers, CallbackInfo ci) {
        try {
            // The data-pack repository is built with the server's built-in
            // packs; the client resource one is not. Use that to pick the type.
            boolean isData = false;
            for (RepositorySource p : providers) {
                if (p instanceof ServerPacksSource) {
                    isData = true;
                    break;
                }
            }
            PackType type = isData ? PackType.SERVER_DATA : PackType.CLIENT_RESOURCES;

            Set<RepositorySource> updated = new HashSet<>(this.sources);
            updated.add(new YogPackProvider(type));
            this.sources = updated;
        } catch (Throwable t) {
            YogLog.warn("could not register pack provider: " + t);
        }
    }
}
