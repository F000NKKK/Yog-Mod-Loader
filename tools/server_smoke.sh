#!/usr/bin/env bash
# A dedicated server of <loader> <mc> started from the dev instance (its
# staged mods included) until it says it's done, then stopped with its own
# `stop`: whether the host and the mods load on a server, where no client
# class may be touched. Leaves the run directory as it found it.
# YOG_SMOKE_COMMANDS (`;`-separated) are typed into its console once it's
# done; their answers end up in the log, printed after.
#
#   tools/server_smoke.sh fabric 1.20.1
#   YOG_SMOKE_COMMANDS="setblock 0 100 0 mymod:lamp" tools/server_smoke.sh fabric 1.21.1
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOADER="${1:?loader}"; MC="${2:?minecraft version}"
WAIT="${YOG_SMOKE_WAIT:-300}"
# Seconds it keeps running once done (its first ticks at least: a crash in
# them fails the run), then stopped.
HOLD="${YOG_SMOKE_HOLD:-5}"
source <(sed -n '/^find_java()/,/^}/p;/^java_for()/,/^}/p;/^gradle_dir()/,/^}/p;/^gradle_in()/,/^}/p;/^die()/,/^}/p' "$ROOT/build.sh")
DEV=1; MC_VERSION="$MC"
# Every loader runs its game in <loader>/run (fabric/modern too: runDir "../run").
RUN="$ROOT/$LOADER/run"
mkdir -p "$RUN"
before="$(ls -A "$RUN" | LC_ALL=C sort)"
CRASHES="$RUN/crash-reports"
crashes_before="$(ls -A "$CRASHES" 2>/dev/null | LC_ALL=C sort)"
echo "eula=true" > "$RUN/eula.txt"
# A port of its own: no clash with a game you run, and the server found by it.
PORT=""
for _ in $(seq 50); do
    p=$((30000 + RANDOM % 20000))
    ss -ltn 2>/dev/null | grep -q ":$p " || { PORT=$p; break; }
done
[ -n "$PORT" ] || { echo "no free port"; exit 2; }
if [ -e "$RUN/server.properties" ]; then
    # A server of yours set up here: its own port, which must be free (the
    # process on it after start is then this one, never another).
    PORT="$(grep -oP '^server-port=\K[0-9]+' "$RUN/server.properties" || echo 25565)"
    if ss -ltn 2>/dev/null | grep -q ":$PORT "; then
        echo "port $PORT is in use: a server runs already"; rm -f "$RUN/eula.txt"; exit 2
    fi
else
    echo "server-port=$PORT" > "$RUN/server.properties"
fi
work="$(mktemp -d)"; log="$work/server.log"; fifo="$work/stdin"
mkfifo "$fifo"
# The server reads its console from the fifo; a writer held open keeps it from ending.
exec 3<>"$fifo"
( gradle_in "$LOADER" runServer --console=plain < "$fifo" > "$log" 2>&1 ) &
pid=$!
ok=1
for _ in $(seq "$WAIT"); do
    if grep -q 'Done (' "$log"; then ok=0; break; fi
    kill -0 "$pid" 2>/dev/null || break
    sleep 1
done
[ "$ok" = 0 ] && sleep "$HOLD"
# Commands for its console once it's done (`;`-separated): what they answer
# is printed after the run.
if [ "$ok" = 0 ] && [ -n "${YOG_SMOKE_COMMANDS:-}" ]; then
    IFS=';' read -ra commands <<< "$YOG_SMOKE_COMMANDS"
    for c in "${commands[@]}"; do echo "$c" >&3; done
    sleep "$HOLD"
fi
# A crash report written on the way (after "Done" too) fails it.
crash="$(comm -13 <(echo "$crashes_before") <(ls -A "$CRASHES" 2>/dev/null | LC_ALL=C sort) | head -1)"
if [ -n "$crash" ]; then
    cp "$CRASHES/$crash" "$work/"; crash="$work/$crash"; ok=1
fi
echo stop >&3
# The game itself (not every loader's Gradle hands it the console): the
# process on its port, stopped as a shutdown stops it, waited for.
server="$(ss -ltnp 2>/dev/null | grep ":$PORT " | grep -o 'pid=[0-9]*' | head -1 | cut -d= -f2)"
# Only a process running in this run directory is ours to stop.
if [ -n "$server" ] && [ "$(readlink -f "/proc/$server/cwd" 2>/dev/null)" != "$(readlink -f "$RUN")" ]; then
    echo "the process on port $PORT isn't this run's: left alone"; server=""
fi
for _ in $(seq 30); do [ -z "$server" ] || ! kill -0 "$server" 2>/dev/null && break; sleep 1; done
if [ -n "$server" ] && kill -0 "$server" 2>/dev/null; then
    kill "$server"
    for _ in $(seq 90); do kill -0 "$server" 2>/dev/null || break; sleep 1; done
    kill -0 "$server" 2>/dev/null && kill -9 "$server"
    for _ in $(seq 10); do kill -0 "$server" 2>/dev/null || break; sleep 1; done
fi
for _ in $(seq 30); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
kill "$pid" 2>/dev/null
wait "$pid" 2>/dev/null
exec 3>&-
for f in $(comm -13 <(echo "$before") <(ls -A "$RUN" | LC_ALL=C sort)); do rm -rf "${RUN:?}/$f"; done
grep -E 'Done \(|ERROR|Cannot load class|Exception' "$log" | grep -v '^\s*at ' | head -20
if [ -n "${YOG_SMOKE_COMMANDS:-}" ]; then
    echo "--- after the commands:"
    sed -n '/Done (/,$p' "$log" | grep -v '^\s*at ' | tail -20
fi
echo "log: $log"
[ -n "$crash" ] && echo "crash report: $crash"
if [ "$ok" = 0 ]; then echo "==> $LOADER $MC: dedicated server OK"; else echo "==> $LOADER $MC: dedicated server FAILED (log: $log)"; fi
exit "$ok"
