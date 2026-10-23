package dev.yog;

import dev.yog.bus.YogTypes;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.particle.ParticleRenderType;
import net.minecraft.client.particle.TextureSheetParticle;
import net.minecraft.client.particle.SpriteSet;
import net.minecraft.util.RandomSource;
import net.minecraft.world.phys.Vec3;

/**
 * A mod's particle, as its {@code .yparticle} says: its frames, its life,
 * its size and opacity over it, how it moves, and how it's drawn
 * ({@link YogParticleLayers}). Its colour is its spawn's.
 */
final class YogParticle extends TextureSheetParticle {
    /** Light at its fullest, sky and block (a glowing particle's). */
    private static final int FULL_BRIGHT = 0xF000F0;
    /** A colour channel's largest value. */
    private static final float CHANNEL = 255f;
    private static final float TURN = (float) (Math.PI * 2);

    private final YogTypes.Particle look;
    private final SpriteSet sprites;
    private final ParticleRenderType layer;

    /** At {@code at}, moving by {@code motion}: one of {@code o}'s, drawn from {@code sprites}. */
    YogParticle(ClientLevel level, Vec3 at, Vec3 motion, YogParticles.Options o, SpriteSet sprites) {
        super(level, at.x, at.y, at.z, motion.x, motion.y, motion.z);
        this.look = o.type().look;
        this.sprites = sprites;
        RandomSource random = level.getRandom();
        int c = o.color();
        setColor((c >> 16 & 0xFF) / CHANNEL, (c >> 8 & 0xFF) / CHANNEL, (c & 0xFF) / CHANNEL);
        quadSize *= look.size();
        alpha = look.alpha();
        friction = look.friction();
        gravity = look.gravity();
        hasPhysics = look.physics();
        speedUpWhenYMotionIsBlocked = look.slideWhenBlocked();
        int min = (int) look.lifetime().min(), max = (int) look.lifetime().max();
        lifetime = min + random.nextInt(Math.max(max - min, 0) + 1);
        if (look.randomRoll()) roll = oRoll = random.nextFloat() * TURN;
        if (YogTypes.Frames.RANDOM.equals(look.frames())) pickSprite(sprites);
        else frame();
        layer = YogParticleLayers.of(look.render());
    }

    @Override
    public void tick() {
        super.tick();
        if (removed) return;
        if (!YogTypes.Frames.RANDOM.equals(look.frames())) frame();
        if (look.fade()) alpha = look.alpha() * (1f - (float) age / lifetime);
        quadSize *= look.sizePerTick();
    }

    /** The frame for its age, `frame_speed` times as fast as it lives. */
    private void frame() {
        setSprite(sprites.get(Math.min((int) (age * look.frameSpeed()), lifetime), lifetime));
    }

    @Override
    public ParticleRenderType getRenderType() {
        return layer;
    }

    @Override
    protected int getLightColor(float partialTick) {
        return look.render().glow() ? FULL_BRIGHT : super.getLightColor(partialTick);
    }
}
