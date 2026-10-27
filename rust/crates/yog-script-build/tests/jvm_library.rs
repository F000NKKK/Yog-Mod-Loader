//! The System library run on the JVM: tasks, LINQ over async sequences,
//! logging, formatting — each source compiled, run, its result checked.

mod support;

/// `Task.WhenAll` of tasks with results: their results, in order, as an
/// array of their type; `WhenAny` the first done.
#[test]
fn when_all_gives_results() {
    let src = "using System.Threading.Tasks;\n\
               public static class R {\n\
                   static async Task<i32> Twice(i32 n) { await Task.Yield(); return n * 2; }\n\
                   public static string Run() {\n\
                       let all = Task.WhenAll(Twice(1), Twice(2), Twice(3)).Result;\n\
                       let names = Task.WhenAll(Task.FromResult(\"a\"), Task.FromResult(\"b\")).Result;\n\
                       let first = Task.WhenAny(Task.FromResult(9)).Result.Result;\n\
                       return all[0] + all[1] + all[2] + \" \" + names[1] + \" \" + first;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("r.ys", src)], "t.RScript.R.Run()") else {
        return;
    };
    assert_eq!(out, "12 b 9");
}

/// LINQ over `IAsyncEnumerable<T>`: lazy filters and projections, values
/// awaited (`CountAsync`, `ToArrayAsync` of the items' own type).
#[test]
fn async_linq() {
    let src = "using System.Linq;\n\
               using System.Threading.Tasks;\n\
               public static class Q {\n\
                   static async IAsyncEnumerable<i32> Numbers(i32 n) {\n\
                       for (let mut i = 1; i <= n; i += 1) { await Task.Yield(); yield return i; }\n\
                   }\n\
                   static async Task<string> Go() {\n\
                       let evens = await Numbers(10).Where(x => x % 2 == 0).Select(x => x * 10).ToArrayAsync();\n\
                       let count = await Numbers(10).Skip(3).Take(4).CountAsync();\n\
                       let first = await Numbers(5).FirstAsync();\n\
                       let any = await Numbers(3).AnyAsync(x => x > 2);\n\
                       let sum = await new i32[] { 1, 2, 3 }.ToAsyncEnumerable().SumAsync();\n\
                       return evens[0] + \",\" + evens[4] + \" \" + count + \" \" + first + \" \" + any + \" \" + sum;\n\
                   }\n\
                   public static string Run() => Go().Result;\n\
               }\n";
    let Some(out) = support::run(&[("q.ys", src)], "t.QScript.Q.Run()") else {
        return;
    };
    assert_eq!(out, "20,100 4 1 True 6");
}

/// `System.Logging`: a factory of a provider, its minimum level and
/// filters, message templates, scopes, `LoggerMessage`, a queued provider.
#[test]
fn logging() {
    let src = "using System.Logging;\n\
               using System.Threading;\n\
               using System.Threading.Tasks;\n\
               public readonly sealed class Sink : ILoggerProvider, ISupportExternalScope {\n\
                   public readonly Lock<List<string>> Lines = new Lock<List<string>>(new List<string>());\n\
                   private readonly Once<IExternalScopeProvider> _scopes = new Once<IExternalScopeProvider>();\n\
                   public readonly ILogger CreateLogger(string categoryName) => new SinkLogger(this, categoryName);\n\
                   public readonly void SetScopeProvider(IExternalScopeProvider scopeProvider) { _scopes.TrySet(scopeProvider); }\n\
                   public readonly string Scopes() {\n\
                       let mut text = \"\";\n\
                       let provider = _scopes.Get();\n\
                       if (provider != null) { provider.ForEachScope((scope, state) => { text = text + \"[\" + scope + \"]\"; }, 0); }\n\
                       return text;\n\
                   }\n\
                   public void Dispose() { }\n\
               }\n\
               public readonly sealed class SinkLogger(Sink sink, string category) : ILogger {\n\
                   public readonly void Log<TState>(LogLevel logLevel, EventId eventId, TState state, Error? error, Func<TState, Error?, string> formatter) {\n\
                       let line = logLevel + \" \" + category + sink.Scopes() + \": \" + formatter(state, error);\n\
                       lock (sink.Lines, ref let lines) { lines.Add(line); }\n\
                   }\n\
                   public readonly bool IsEnabled(LogLevel logLevel) => true;\n\
                   public readonly IDisposable? BeginScope<TState>(TState state) => null;\n\
               }\n\
               public readonly sealed class Memory : QueuedLoggerProvider {\n\
                   public readonly Lock<List<string>> Written = new Lock<List<string>>(new List<string>());\n\
                   protected override async Task WriteAsync(QueuedEntry entry) {\n\
                       await Task.Yield();\n\
                       lock (Written, ref let w) { w.Add(entry.Message); }\n\
                   }\n\
               }\n\
               public static class L {\n\
                   public static string Run() {\n\
                       let sink = new Sink();\n\
                       let factory = LoggerFactory.Create(b => {\n\
                           b.AddProvider(sink);\n\
                           b.SetMinimumLevel(LogLevel.Debug).AddFilter(\"Noisy\", LogLevel.Error);\n\
                       });\n\
                       let log = factory.CreateLogger(\"App\");\n\
                       log.LogTrace(\"hidden\");\n\
                       log.LogInformation(\"Moved {Count} items to {Target}\", 3, \"chest\");\n\
                       let count = 4;\n\
                       let target = \"barrel\";\n\
                       let weight = 2.25;\n\
                       log.LogInformation(\"Moved {count} items to {target}, {weight,6:F1} kg\");\n\
                       using (let scope = log.BeginScope(\"Request {Id}\", 7)) {\n\
                           log.LogWarning(\"inside\");\n\
                       }\n\
                       factory.CreateLogger(\"Noisy.Part\").LogWarning(\"dropped\");\n\
                       let said = LoggerMessage.Define<i32>(LogLevel.Error, 5, \"Failed {Code}\");\n\
                       said(log, 42, null);\n\
                       let memory = new Memory();\n\
                       let queued = memory.CreateLogger(\"Q\");\n\
                       queued.LogInformation(\"one {N}\", 1);\n\
                       queued.LogInformation(\"two\");\n\
                       memory.FlushAsync().Wait();\n\
                       let mut text = \"\";\n\
                       lock (sink.Lines, ref let lines) { text = string.Join(\" | \", lines); }\n\
                       lock (memory.Written, ref let w) { text = text + \" || \" + string.Join(\",\", w); }\n\
                       return text + \" || \" + NullLogger.Instance.IsEnabled(LogLevel.Critical);\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("l.ys", src)], "t.LScript.L.Run()") else {
        return;
    };
    assert_eq!(
        out,
        "Information App: Moved 3 items to chest | Information App: Moved 4 items to barrel,    2.3 kg | Warning App[Request 7]: inside | Error App: Failed 42 || one 1,two || False"
    );
}

/// The game's log as a provider: added by `AddGameLog`, aliased `Game` in
/// the rules (its writing is the game's, not run here).
#[test]
fn game_log_provider() {
    let src = "using System.Logging;\n\
               using Yog;\n\
               public static class G {\n\
                   public static string Run() {\n\
                       let factory = LoggerFactory.Create(b => {\n\
                           b.AddGameLog().AddFilter((provider, category, level) => provider != null);\n\
                       });\n\
                       let log = factory.CreateLogger(\"Pipes\");\n\
                       return \"made\";\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("g.ys", src)], "t.GScript.G.Run()") else {
        return;
    };
    assert_eq!(out, "made");
}

/// .NET's formatting: `{x,5:F2}` holes, numbers' `ToString(format)`
/// (standard and custom), `string.Format`.
#[test]
fn formatting() {
    let src = "public static class F {\n\
                   public static string Run() {\n\
                       let x = 1234.5678;\n\
                       let n = 42;\n\
                       let a = \"[{x:F2}] [{n,5}] [{n,-5}] [{n:X4}] [{-1:X}] [{x:N1}] [{0.256:P1}] [{x:E3}] [{n:D5}] [{x:0.0}] [{x:#,##0}]\";\n\
                       let b = string.Format(\"\"\"{0} + {1,4:F1} = {{{2}}}\"\"\", n, 2.25, true);\n\
                       let c = 255.ToString(\"x\") + \" \" + (12345678901i64).ToString(\"N0\") + \" \" + ((decimal)0.5).ToString(\"F3\") + \" \" + 1.5e-7.ToString(\"G\");\n\
                       return a + \" | \" + b + \" | \" + c;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("f.ys", src)], "t.FScript.F.Run()") else {
        return;
    };
    assert_eq!(
        out,
        "[1234.57] [   42] [42   ] [002A] [FFFFFFFF] [1,234.6] [25.6 %] [1.235E+003] [00042] [1234.6] [1,235] | 42 +  2.3 = {True} | ff 12,345,678,901 0.500 1.5E-07"
    );
}

/// `await using`: the value's `DisposeAsync` awaited where what follows
/// ends — its block's end, an early `return`, a body's end; `null` skipped.
#[test]
fn await_using() {
    let src = "using System.Threading;\n\
               using System.Threading.Tasks;\n\
               public readonly sealed class Res(string name, Lock<List<string>> log) : IAsyncDisposable {\n\
                   public async Task DisposeAsync() {\n\
                       await Task.Yield();\n\
                       lock (log, ref let l) { l.Add(name); }\n\
                   }\n\
               }\n\
               public static class U {\n\
                   static async Task<i32> Use(Lock<List<string>> log, bool early) {\n\
                       await using let a = new Res(\"a\", log);\n\
                       await using (let b = new Res(\"b\", log)) {\n\
                           lock (log, ref let l) { l.Add(\"body\"); }\n\
                       }\n\
                       Res? none = null;\n\
                       await using let c = none;\n\
                       if (early) {\n\
                           return 1;\n\
                       }\n\
                       lock (log, ref let l) { l.Add(\"end\"); }\n\
                       return 2;\n\
                   }\n\
                   public static string Run() {\n\
                       let log = new Lock<List<string>>(new List<string>());\n\
                       let x = Use(log, true).Result;\n\
                       let y = Use(log, false).Result;\n\
                       let mut text = \"\";\n\
                       lock (log, ref let l) { text = string.Join(\",\", l); }\n\
                       return x + \" \" + y + \" \" + text;\n\
                   }\n\
               }\n";
    let Some(out) = support::run(&[("u.ys", src)], "t.UScript.U.Run()") else {
        return;
    };
    assert_eq!(out, "1 2 body,b,a,body,b,end,a");
}
