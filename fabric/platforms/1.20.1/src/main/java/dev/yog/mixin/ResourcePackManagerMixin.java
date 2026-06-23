package dev.yog.mixin;

import java.util.HashSet;
import java.util.Set;
import dev.yog.YogPackProvider;
import net.minecraft.resource.ResourcePackManager;
import net.minecraft.resource.ResourcePackProvider;
import net.minecraft.resource.ResourceType;
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
@Mixin(ResourcePackManager.class)
public class ResourcePackManagerMixin {
    @Shadow
    @Final
    @Mutable
    private Set<ResourcePackProvider> providers;

    @Inject(method = "<init>", at = @At("TAIL"))
    private void yog$addProvider(ResourcePackProvider[] providers, CallbackInfo ci) {
        try {
            // The data-pack manager is constructed with a *DataPack* provider;
            // the client resource manager is not. Use that to pick the type.
            boolean isData = false;
            for (ResourcePackProvider p : providers) {
                if (p.getClass().getName().contains("DataPack")) {
                    isData = true;
                    break;
                }
            }
            ResourceType type = isData ? ResourceType.SERVER_DATA : ResourceType.CLIENT_RESOURCES;

            Set<ResourcePackProvider> updated = new HashSet<>(this.providers);
            updated.add(new YogPackProvider(type));
            this.providers = updated;
        } catch (Throwable t) {
            System.err.println("[yog] could not register pack provider: " + t);
        }
    }
}
