# Design: ownership, sharing, synchronization and threads in Yog Script

Status: design, for review. Scope: the System library (`System`,
`System.Threading`, `System.Threading.Tasks`, `System.Collections.Concurrent`),
the checker (sendability), the JVM runtime, Rust lowering.

The user's ask (2026-10-26):
- Take Rust's primitives (`Box`, `Rc`, `Arc`, `Weak`, `Cell`, `RefCell`,
  `Mutex`, `RwLock`, atomics, channels) and C#'s (`lock`/`Monitor`,
  `Interlocked`, `SemaphoreSlim`, `Lazy`, `ThreadLocal`, `AsyncLocal`,
  `Concurrent*`, `Channel<T>`, `CancellationToken`).
- Keep the best set. Drop what's redundant, or express it through the
  others, but leave full freedom.
- `Thread` must be there.

## 0. Principles

1. **One concept, one type.** Where Rust has two types only for speed
   (`Rc`/`Arc`, `Cell`/`RefCell`, `Mutex`/`lock`), YS has one, and the
   compiler picks the fast form. The checker knows what crosses a thread;
   the lowering uses that.
2. **Data-owning locks** (Rust's): a lock *holds* what it guards, so code
   can't touch the data without holding it. C#'s `lock (obj) { … }` is a
   statement over such a lock, not a lock on any object.
3. **Sendability is inferred, never written.** A value going to another
   thread (a thread's lambda, a channel's item, a `Shared` read on two
   threads) must be safe to send or share. The checker infers this the way
   Rust's auto traits work, with no `Send`/`Sync` keywords. A violation is a
   build error naming the field that makes it unsafe.
4. **Full freedom.** Every capability of both worlds stays reachable:
   memory orderings, scoped threads, condition variables,
   reader/writer locks, spinning through atomics, and an `unsafe` region
   that opts out of the checks.

## 1. The set

### 1.1 Ownership and heap

| YS | From | Why, and what it replaces |
|---|---|---|
| `Box<T>` | Rust `Box` | One owner, on the heap: a recursive struct or enum, a big struct moved in O(1), a value behind an interface without copying. C#'s implicit boxing of a struct as `object` stays and copies. |
| `Shared<T>` | Rust `Rc` + `Arc` | Several owners, cheap `Clone()`. One type: the lowering uses `Rc` unless the checker sees it cross a thread, then `Arc`. On the JVM it's a plain reference, since the GC owns the memory. A cycle is collected by the GC. |
| `Weak<T>` | Rust `Weak`, C# `WeakReference<T>` | Doesn't keep its value: `TryGet()` gives `T?`. |

Dropped: `Cow<T>` (`string` is immutable and shared already; anything
else is an enum of borrowed or owned), `Pin` (no self-referential futures
in user code, and the GC moves nothing a mod sees).

### 1.2 Changing what's shared

| YS | From | Members |
|---|---|---|
| `Cell<T>` | Rust `Cell` + `RefCell` | `Get()`/`Set(v)` for a `Copy` `T`; `Replace(v)`, `Take()`, `Update(f)` for any; `Borrow()`/`BorrowMut()` give guards checked at run time (a panic on a conflicting borrow). One thread only: the checker refuses one that crosses threads. Lowered to `Cell` when only `Get`/`Set` are used, else `RefCell`. |
| `Lock<T>` | Rust `Mutex`, C# `lock`/`Monitor` | `Lock()` gives a guard (`using`: dropped at the end of its scope), `TryLock()` gives `Guard?`, `TryLock(TimeSpan)`, `LockAsync()` for tasks; condition variables `Wait(ref guard)`, `Wait(ref guard, TimeSpan)`, `Notify()`, `NotifyAll()` (Monitor's `Wait`/`Pulse`, Rust's `Condvar`). Statement form: `lock (counter, ref let n) { n += 1; }`. |
| `ReadWriteLock<T>` | Rust `RwLock`, C# `ReaderWriterLockSlim` | `Read()`, `Write()`, `UpgradeableRead()` (C#'s, to write without letting go), and the `Try…` and `…Async` forms. |
| `Atomic<T>` | Rust `Atomic*`, C# `Interlocked`, `Volatile` | `T`: any integer, `bool`, `char`, a reference type, a `Copy` enum. `Load`, `Store`, `Exchange`, `CompareExchange`, `Add`/`Sub`/`And`/`Or`/`Xor`/`Max`/`Min`, `Increment`/`Decrement`, `Update(f)` (a CAS loop). Each takes an optional `MemoryOrder` (`Relaxed`, `Acquire`, `Release`, `AcqRel`, `SeqCst`, the default). C#'s `volatile` field is `Atomic<T>` used with `Acquire`/`Release`. |
| `Once<T>` | Rust `OnceCell`/`OnceLock`, C# `LazyInitializer` | Set once: `TrySet(v)`, `GetOrInit(f)`, `Get()` gives `T?`. Thread-safe; on one thread it's lowered to `OnceCell`. |
| `Lazy<T>` | C# `Lazy<T>`, Rust `LazyLock` | `Once<T>` plus its factory, which is how it's written in the library. |

Dropped, expressed through the above: `SpinLock`/`SpinWait` (a loop over
`Atomic.CompareExchange` with `Thread.SpinWait(n)`), C#'s `Monitor.Enter`
on an arbitrary object (use a `Lock<T>`), `Interlocked` as a static class
(use `Atomic<T>`).

### 1.3 Waiting and signalling

| YS | From | What |
|---|---|---|
| `Semaphore` | C# `SemaphoreSlim`/`Semaphore`, tokio's | `Wait()`, `Wait(TimeSpan)`, `WaitAsync()`, `Release(n)`, `CurrentCount`. |
| `Signal` | C# `ManualResetEventSlim`, `AutoResetEvent` | `Set()`, `Reset()`, `Wait()`, `WaitAsync()`; `new Signal(autoReset: true)` is an `AutoResetEvent`. |
| `Countdown` | C# `CountdownEvent` | `Signal()`, `AddCount()`, `Wait()`, `WaitAsync()`. |
| `Barrier` | C# `Barrier`, Rust `Barrier` | `SignalAndWait()`, with phases. |
| `CancellationTokenSource` / `CancellationToken` | C# | `Cancel()`, `CancelAfter(TimeSpan)`, `IsCancellationRequested`, `Register(f)`, `ThrowIfCancellationRequested()` (panics). `Linked(a, b)`. Every waiting member takes one. |

### 1.4 Passing values between threads and tasks

| YS | From | What |
|---|---|---|
| `Channel<T>` | C# `System.Threading.Channels`, Rust `mpsc`/crossbeam, `BlockingCollection` | `Channel.Bounded<T>(n, full: Wait/DropOldest/DropNewest)`, `Channel.Unbounded<T>()`; a `Writer` (`Write`, `TryWrite`, `WriteAsync`, `Complete()`) and a `Reader` (`Read`, `TryRead`, `ReadAsync`, `ReadAllAsync()` for `await foreach`, `Completion`). Many writers, many readers. Items are moved in. |
| `IAsyncEnumerable<T>` / `IAsyncEnumerator<T>` | C# | A sequence read with `await foreach`, written as an `async` iterator (`async IAsyncEnumerable<T> F() { … yield return x; … await … }`); `WithCancellation(token)`; Linq over it (`System.Linq.AsyncEnumerable`). |
| `ConcurrentDictionary<K, V>` | C# | `GetOrAdd`, `AddOrUpdate`, `TryRemove`, … as .NET's. |
| `ConcurrentQueue<T>` | C# | `Enqueue`, `TryDequeue`, `TryPeek`. |

Dropped: `ConcurrentBag`/`ConcurrentStack` (a `Lock<List<T>>`), `BlockingCollection` (`Channel.Bounded`).

### 1.5 Per thread, per flow

| YS | From | What |
|---|---|---|
| `ThreadLocal<T>` | C# `ThreadLocal<T>`, Rust `thread_local!` | A value per thread, made by its factory on first use; `Values` when tracked. `[ThreadStatic]` on a static field is the same, written shorter. |
| `AsyncLocal<T>` | C# `AsyncLocal<T>` | A value that flows with the logical call: into tasks started from it and across `await`s. Logging scopes use it. |

### 1.6 Threads and tasks

```ys
// A thread of its own: its lambda owns what it captures (`move`).
let worker = Thread.Run(move () => Crunch(data));        // ThreadHandle<i64>
i64 sum = worker.Join();                                  // its value; its panic re-raised

// Scoped threads borrow the caller's locals: all joined at the scope's end.
Thread.Scope(s => {
    s.Run(() => Left(in grid));
    s.Run(() => Right(in grid));
});

new Thread(Loop) { Name = "yog-pump", IsBackground = true, Priority = ThreadPriority.Low }.Start();
Thread.Sleep(TimeSpan.FromMilliseconds(5));
Thread.Yield();
```

| YS | From | What |
|---|---|---|
| `Thread` | C# `Thread`, Rust `std::thread` | `new Thread(f)`, `Start()`, `Join()`, `Join(TimeSpan)`, `Name`, `IsBackground`, `Priority`, `ManagedThreadId`, `Thread.CurrentThread`, `Sleep`, `Yield`, `SpinWait(n)`. `Thread.Run(f)` gives a `ThreadHandle<T>` whose `Join()` gives `f`'s value (Rust's `JoinHandle`). `Thread.Scope(f)` runs scoped threads that may borrow (Rust's `thread::scope`). |
| `ThreadPool` | C#, rayon | `QueueUserWorkItem`, and the pool `Task.Run` uses. |
| `Task`, `Task<T>` | C# (already in the library) | Tasks run on the thread that started them (its scheduler). `Task.Run(f)` runs `f` on the pool and is awaited back on the caller's thread: the way off the game thread for heavy work. |
| `Parallel` | C# `Parallel.For/ForEach`, rayon | Over the pool. The checker requires the body's captures to be shareable. |

Game time (`Yog.GameTime`) stays Yog's: it is ticks of a side's game
thread. Everything above is real time, and none of it depends on Yog.

## 2. Checked: what crosses a thread

- **Sendable**: a value whose ownership can move to another thread. That
  covers `Copy` values, owned objects whose fields are all sendable,
  `Shared<T>` of a shareable `T`, channels' ends, locks, atomics and
  handles. Not sendable: `Cell<T>`, a borrow, or anything holding one.
- **Shareable**: what two threads may reach at once. That covers
  immutable values (records, `string`, `readonly` types), `Lock<T>`,
  `ReadWriteLock<T>`, `Atomic<T>`, `Once<T>`, concurrent collections, and
  `Shared<T>` of a shareable `T`.
- **Where it's checked**: the captures of `Thread.Run`, `Thread.Scope`'s
  `s.Run` (borrows allowed, scoped), `Task.Run`, `Parallel`, and
  `ThreadPool`; a `Channel<T>`'s `T`; a `Shared<T>` or a static reached
  from such code.
- **Errors** name the field that makes a value unsendable or unshareable,
  e.g. "`Cache.Hits` is a `Cell<i32>`: one thread only — make it an
  `Atomic<i32>`".
- **`unsafe { … }`** lets a block skip these checks for code that knows
  better. It's reported as a warning wherever it's used.
- **Statics** reached from more than one thread must be shareable, or be
  `[ThreadStatic]`/`ThreadLocal`.

## 3. Lowering

| YS | JVM | Rust |
|---|---|---|
| `Box<T>` | the reference | `Box<T>` |
| `Shared<T>` | the reference | `Rc<T>`, or `Arc<T>` where it crosses a thread |
| `Weak<T>` | `java.lang.ref.WeakReference` | `rc::Weak`/`sync::Weak` |
| `Cell<T>` | a field (with a borrow flag when borrowed) | `Cell`/`RefCell` |
| `Lock<T>` | `ReentrantLock` + `Condition` | `yog_sync::Mutex` + `Condvar` |
| `ReadWriteLock<T>` | `ReentrantReadWriteLock` | `yog_sync::RwLock` |
| `Atomic<T>` | `VarHandle` (each ordering) / `Atomic*` | `std::sync::atomic` |
| `Once<T>`, `Lazy<T>` | a `VarHandle` CAS on a field | `OnceCell`/`OnceLock`, `LazyLock` |
| `Semaphore`, `Signal`, `Countdown`, `Barrier` | `java.util.concurrent` | in `yog-sync`, over `Mutex`/`Condvar` |
| `Channel<T>` | `java.util.concurrent` queues + `CompletableFuture` waiters | in `yog-sync` (crossbeam-like, no new dependency without approval) |
| `Thread` | `java.lang.Thread` (`Thread.ofVirtual()` for the pool's) | `std::thread` (scoped: `thread::scope`) |
| `ThreadLocal`, `AsyncLocal` | `ThreadLocal`; `AsyncLocal` carried by the scheduler's task context | `thread_local!`; the task context |

Every Rust lowering goes through `yog-sync`, the only sync crate Yog code
uses (CLAUDE.md, dedup + yog-sync).

## 4. Steps

1. Library declarations and JVM runtime: `Box`, `Shared`, `Weak`,
   `Cell`, `Lock` (+ `lock` statement), `ReadWriteLock`, `Atomic`,
   `Once`/`Lazy`, `Thread` (+ `Run`, `Scope`), `ThreadLocal`,
   `CancellationToken`. JVM tests for each (`tests/jvm.rs`).
2. `IAsyncEnumerable<T>`, `await foreach`, async iterators; `Channel<T>`,
   `Semaphore`, `Signal`, `Countdown`, `Barrier`,
   `ConcurrentDictionary`, `ConcurrentQueue`, `AsyncLocal`, `Task.Run`,
   `ThreadPool`, `Parallel`.
3. The checker's sendable/shareable inference, errors and `unsafe`.
4. Rust lowering of the set (with step 3 of the plan: Rust lowering).

## 5. Decided

Taken as recommended (2026-10-27), each easy to change:
- `lock` comes in both forms, `lock (x, ref let v) { … }` and `using (let g = x.Enter())`.
- `Shared<T>` is one type, and the compiler picks `Rc` or `Arc`.
- `unsafe` exists, with a warning where it's used.

## 6. Open questions for review

- `lock (x, ref let v) { … }`: is this the statement form you want, or is
  `using (let g = x.Lock()) { g.Value += 1; }` enough?
- `Shared<T>` as one type for both `Rc` and `Arc`, chosen by the
  compiler: agreed, or do you want an explicit `Arc<T>` too?
- Should `unsafe` exist at all, or should the checks always hold?
