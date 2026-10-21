package dev.yog.util;

import dev.yog.bus.YogEvents;
import dev.yog.bus.YogTypes;

/**
 * Bodies, the camera and the travel step through the bus ({@code
 * client/render/body}, {@code client/render/camera}, {@code
 * client/game/travel}): the hosts ask here each frame or tick, the mods'
 * handlers answer — the part every host shares.
 */
public final class YogBodyBus {
    private YogBodyBus() {}

    /** No ops: what a body nobody changes gets. */
    public static final YogTypes.BodyOps NONE = new YogTypes.BodyOps(null, java.util.List.of(), java.util.List.of(), null, false, java.util.List.of());
    private static final YogTypes.Vec3 ZERO = new YogTypes.Vec3(0, 0, 0);

    /** The ops for a tracked entity's body this frame ({@code yog_body::wire}); {@link #NONE} for none. */
    public static YogTypes.BodyOps bodyOps(YogTypes.BodyFrame frame) {
        YogTypes.BodyOps ops = new YogEvents.Client.Render.Body(frame, NONE).emitChanged().ops();
        return ops.equals(NONE) ? NONE : ops;
    }

    /** {right, up, forward, wx, wy, wz, yaw, pitch, roll, fov} the mods make of this frame's camera. */
    public static float[] camera(YogTypes.CameraFrame frame) {
        YogTypes.CameraOut o = new YogEvents.Client.Render.Camera(frame, new YogTypes.CameraOut(ZERO, ZERO, 0, 0, 0, 0)).emitChanged().out();
        return new float[] {
            (float) o.localOffset().x(), (float) o.localOffset().y(), (float) o.localOffset().z(),
            (float) o.worldOffset().x(), (float) o.worldOffset().y(), (float) o.worldOffset().z(),
            (float) o.yaw(), (float) o.pitch(), (float) o.roll(), (float) o.fov(),
        };
    }

    /** The velocity a mod moves the local player by this tick instead of vanilla's travel, or null. */
    public static float[] travel(YogTypes.TravelFrame frame) {
        if (!YogEvents.Client.Game.Travel.heard()) return null;
        YogTypes.Vec3 v = new YogEvents.Client.Game.Travel(frame, null).emitChanged().velocity();
        return v == null ? null : new float[] {(float) v.x(), (float) v.y(), (float) v.z()};
    }
}
