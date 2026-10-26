package dev.yog.script.threading.tasks;

/** What a task completion source's tasks fail with. */
final class Completions {
    private Completions() {}

    /** What setting a task settled already panics with. */
    static RuntimeException settledAlready() {
        return new dev.yog.script.YsPanic("the task is settled already", "");
    }

    /** A task's failure of Yog Script's {@code error}: awaiting it panics with its message. */
    static RuntimeException failure(dev.yog.script.Error error) {
        return new dev.yog.script.YsPanic(error.Message, "");
    }
}
