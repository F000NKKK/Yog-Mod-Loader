package dev.yog.script.threading;

import java.util.concurrent.Phaser;

/** Yog Script's {@code System.Threading.Barrier}: threads meeting, phase after phase. */
public final class Barrier {
    private final Phaser phaser;

    public Barrier(int participantCount) {
        this.phaser = new Phaser(participantCount);
    }

    public void SignalAndWait() {
        phaser.arriveAndAwaitAdvance();
    }

    public int ParticipantCount() {
        return phaser.getRegisteredParties();
    }

    public long CurrentPhaseNumber() {
        return phaser.getPhase();
    }
}
