package dev.yog.mixin;

import java.util.Map;
import net.minecraft.client.model.geom.ModelPart;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A part's children by name (yog-body poses parts by name). */
@Mixin(ModelPart.class)
public interface ModelPartAccess {
    @Accessor("children")
    Map<String, ModelPart> yog$children();
}
