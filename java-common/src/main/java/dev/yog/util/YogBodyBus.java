package dev.yog.util;

import dev.yog.bus.YogEvents;
import dev.yog.bus.YogTypes;
import java.util.List;

/**
 * Bodies, the camera and the travel step through the bus ({@code
 * client/render/body}, {@code client/render/camera}, {@code
 * client/game/travel}): the hosts ask here each frame or tick, the mods'
 * handlers answer — the part every host shares.
 */
public final class YogBodyBus {
    private YogBodyBus() {}

    private static final byte[] NONE = new byte[0];
    private static final YogTypes.Vec3 ZERO = new YogTypes.Vec3(0, 0, 0);

    /** The ops for a tracked entity's body this frame ({@code yog_body::wire}), or null for none. */
    public static byte[] bodyPose(byte[] frame) {
        byte[] ops = new YogEvents.Client.Render.Body(frame, NONE).emitChanged().ops();
        return ops.length == 0 ? null : ops;
    }

    /** {right, up, forward, wx, wy, wz, yaw, pitch, roll, fov} the mods make of this frame's camera. */
    public static float[] camera(String uuid, float partial, double x, double y, double z, float yaw, float pitch, boolean detached, int flags) {
        YogTypes.CameraFrame frame = new YogTypes.CameraFrame(uuid, partial, List.of(x, y, z), yaw, pitch, detached, flags);
        YogTypes.CameraOut o = new YogEvents.Client.Render.Camera(frame, new YogTypes.CameraOut(ZERO, ZERO, 0, 0, 0, 0)).emitChanged().out();
        return new float[] {
            (float) o.localOffset().x(), (float) o.localOffset().y(), (float) o.localOffset().z(),
            (float) o.worldOffset().x(), (float) o.worldOffset().y(), (float) o.worldOffset().z(),
            (float) o.yaw(), (float) o.pitch(), (float) o.roll(), (float) o.fov(),
        };
    }

    /** The velocity a mod moves the local player by this tick instead of vanilla's travel, or null. */
    public static float[] travel(String uuid, float vx, float vy, float vz, float lx, float ly, float lz, float yaw, float pitch,
                                 float forward, float strafe, boolean jumping, boolean sneaking, float gravity, int flags) {
        if (!YogEvents.Client.Game.Travel.heard()) return null;
        YogTypes.TravelFrame frame = new YogTypes.TravelFrame(uuid, new YogTypes.Vec3(vx, vy, vz), new YogTypes.Vec3(lx, ly, lz),
                yaw, pitch, forward, strafe, jumping, sneaking, gravity, flags);
        YogTypes.Vec3 v = new YogEvents.Client.Game.Travel(frame, null).emitChanged().velocity();
        return v == null ? null : new float[] {(float) v.x(), (float) v.y(), (float) v.z()};
    }
}
