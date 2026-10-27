package dev.yog.script.yog;

import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.bus.YogEvents;
import dev.yog.bus.YogServices;
import java.util.List;
import java.util.function.BiPredicate;

/**
 * A {@code command} of Yog Script run: each of its methods a way of running
 * it ({@code /charge fill <target>}), declared to the host with its
 * arguments' types, answered when a run names it and fits it. What it
 * {@code requires} refuses the rest; its panic is told, the game going on.
 */
public final class Commands {
    /** What a refused run is told. */
    private static final String REFUSED = "You can't run this.";
    /** What separates a command's schema's words. */
    private static final String SCHEMA_SEP = " ";

    private Commands() {}

    /**
     * Command {@code name}'s way {@code way} taking arguments of {@code types}
     * ({@link YogNames.CommandArg}): declared, its runs answered by {@code run}
     * (false: what it requires doesn't hold).
     */
    public static void declare(String name, String way, String[] types, BiPredicate<Sender, List<String>> run) {
        StringBuilder schema = new StringBuilder(YogNames.CommandArg.WORD);
        for (String t : types) {
            schema.append(SCHEMA_SEP).append(t);
        }
        new YogServices.Command.CommandSpec(name, schema.toString()).call();
        YogEvents.Server.Command.Run.on(name, 0, r -> {
            List<String> args = r.args();
            boolean fits = r.reply() == null && args.size() == types.length + 1 && args.get(0).equalsIgnoreCase(way);
            if (!fits) {
                return null;
            }
            Sender s = new Sender(r.source(), r.uuid(), r.dimension(), r.permission(), r.op());
            try {
                if (!run.test(s, args.subList(1, args.size()))) {
                    s.Reply(REFUSED);
                }
            } catch (RuntimeException e) {
                YogLog.error("Yog Script: /" + name + " " + way + " panicked: " + e.getMessage());
                s.Reply(e.getMessage());
            }
            return new YogEvents.Server.Command.Run(r.name(), r.args(), r.source(), r.uuid(), r.dimension(), r.permission(), r.op(), s.reply());
        });
    }
}
