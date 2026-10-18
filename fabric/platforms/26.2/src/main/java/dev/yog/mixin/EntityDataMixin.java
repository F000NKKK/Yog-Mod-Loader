package dev.yog.mixin;

import com.mojang.serialization.Codec;
import dev.yog.NativeBridge;
import java.nio.ByteBuffer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.storage.ValueInput;
import net.minecraft.world.level.storage.ValueOutput;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Entity data a mod marked {@code persist} is saved and loaded with the entity. */
@Mixin(Entity.class)
public abstract class EntityDataMixin {
    private static final String KEY = "yog:data";

    @Inject(method = "saveWithoutId", at = @At("RETURN"))
    private void yog$save(ValueOutput out, CallbackInfo ci) {
        Entity self = (Entity) (Object) this;
        if (self.level() == null || self.level().isClientSide()) return;
        byte[] data = NativeBridge.nativeEntityDataSave(self.getStringUUID());
        if (data != null) out.store(KEY, Codec.BYTE_BUFFER, ByteBuffer.wrap(data));
    }

    @Inject(method = "load", at = @At("TAIL"))
    private void yog$load(ValueInput in, CallbackInfo ci) {
        Entity self = (Entity) (Object) this;
        if (self.level() == null || self.level().isClientSide()) return;
        in.read(KEY, Codec.BYTE_BUFFER).ifPresent(buf -> {
            byte[] data = new byte[buf.remaining()];
            buf.get(data);
            NativeBridge.nativeEntityDataLoad(self.getStringUUID(), data);
        });
    }
}
