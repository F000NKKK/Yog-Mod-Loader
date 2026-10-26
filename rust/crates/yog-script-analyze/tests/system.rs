//! The System library: its own sources check, and code uses it.

use yog_ids::script_problem;
use yog_script_analyze::ast::DeclKind;
use yog_script_analyze::{check, system};

fn codes(src: &str) -> Vec<String> {
    check("a.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn the_librarys_sources_check() {
    assert!(!system::sources().is_empty());
    for (path, src) in system::sources() {
        let checked = check(path, src);
        assert!(
            checked.problems.is_empty(),
            "{path}: {:#?}",
            checked.problems
        );
        // As .NET's IDE0130: a file's namespace is its folders.
        let folders = path
            .rsplit_once(yog_script_ids::system::FOLDER_SEP)
            .map_or("", |(f, _)| f);
        let declared = checked.file.decls.iter().find_map(|d| match &d.kind {
            DeclKind::Namespace { path, .. } => Some(
                path.iter()
                    .map(|n| n.text.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            ),
            _ => None,
        });
        assert_eq!(
            declared.as_deref(),
            Some(
                folders
                    .replace(
                        yog_script_ids::system::FOLDER_SEP,
                        yog_script_ids::system::NAMESPACE_SEP
                    )
                    .as_str()
            ),
            "{path}"
        );
    }
}

#[test]
fn results_are_rusts() {
    let ok = "Result<i32, Error> Parse(string s) => Ok(4);\n\
              Result<i32, Error> Twice(string s) { let v = try Parse(s); return Ok(v * 2); }\n\
              i32 Max() => Math.Max(1, 2);\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // `try` gives the value's type; a case's data is checked.
    let bad = "Result<i32, Error> Parse(string s) => Ok(\"x\");\n\
               string Twice(string s) { string v = try Parse(s); return v; }\n";
    assert_eq!(
        codes(bad),
        [script_problem::TYPE_MISMATCH, script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn attributes_are_attribute_classes() {
    let ok = "[Obsolete(\"use G\")]\nvoid F() {}\nclass Range(i32 min, i32 max) : Attribute {}\n[Range(0, 10)]\nvoid H() {}\n[Derive(Clone)]\nclass Box {}\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    let bad = "class NotOne {}\n[NotOne]\nvoid F() {}\n[Obsolete(3)]\nvoid G() {}\n";
    assert_eq!(
        codes(bad),
        [script_problem::ATTRIBUTE, script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn rust_is_reached_on_the_bus() {
    let ok = "using System.Yog.Native;\n[Service(\"charge:simulate\")]\nstatic extern i64 Simulate(i64 nodes);\ni64 F() => Simulate(3);\n\
              [Provide(\"charge:double\")]\nstatic i64 Double(i64 x) => x * 2;\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    assert_eq!(
        codes("static extern i64 Simulate(i64 nodes);\n"),
        [script_problem::ATTRIBUTE]
    );
}

#[test]
fn time_spans_add_and_compare_by_their_operators() {
    let ok = "TimeSpan Wait() => TimeSpan.FromSeconds(2) + TimeSpan.FromMilliseconds(500);\n\
              bool Long(TimeSpan t) => t * 2.0 >= TimeSpan.FromMinutes(1) && -t < TimeSpan.Zero;\n\
              f64 Halves(TimeSpan t) => t / TimeSpan.FromSeconds(0.5);\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // Two spans don't multiply; a span isn't a number.
    let bad = "TimeSpan F(TimeSpan t) => t * t;\nf64 G(TimeSpan t) => t;\n";
    assert_eq!(
        codes(bad),
        [script_problem::TYPE_MISMATCH, script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn vectors_are_numerics_by_their_operators() {
    let ok = "using System.Numerics;\n\
              Vector3 Step(Vector3 from, Vector3 to, f32 speed) => from + Vector3.Normalize(to - from) * speed;\n\
              bool Still(Vector2 v) => -v == Vector2.Zero;\n\
              f32 Far(Vector3 a, Vector3 b) => Math.Max(Vector3.Distance(a, b), 1);\n";
    assert!(codes(ok).is_empty(), "{:?}", codes(ok));
    // A vector isn't a number; `System.Numerics` is `using`'d.
    let bad = "using System.Numerics;\nf32 F(Vector3 v) => v + 1;\n";
    assert_eq!(codes(bad), [script_problem::TYPE_MISMATCH]);
    assert_eq!(
        codes("Vector3 F() => default;\n"),
        [script_problem::UNKNOWN_TYPE]
    );
    // Written in full, without the `using`: its members' types are read in
    // its own namespace, not the caller's.
    let full = "System.Numerics.Vector3 F(System.Numerics.Vector3 v) =>\n\
                System.Numerics.Vector3.Normalize(v) + v;\n";
    assert!(codes(full).is_empty(), "{:?}", codes(full));
}

#[test]
fn collections_are_the_librarys_own() {
    let ok = "i32 Sum(List<i32> xs) {\n    let mut total = 0;\n    foreach (let x in xs) {\n        total += x;\n    }\n    return total + xs.Count + xs[0];\n}\n\
              bool Has(Dictionary<string, i32> ages, string name) => ages.ContainsKey(name) && ages[name] > 3;\n\
              i32? Age(Dictionary<string, i32> ages) => ages.TryGetValue(\"steve\");\n\
              void Fill(List<string> names) {\n    names.Add(\"a\");\n    names.Insert(0, \"b\");\n    names.RemoveAll(n => n == \"a\");\n    names.Sort((a, b) => a.Length - b.Length);\n}\n\
              i32 Firsts(in IEnumerable<i32> xs) {\n    foreach (let x in xs) {\n        return x;\n    }\n    return 0;\n}\n\
              i32 Pass(List<i32> xs) => Firsts(in xs);\n\
              List<i32> Made() => [1, 2, 3];\n\
              HashSet<string> Set() => [\"a\", \"b\"];\n\
              Dictionary<string, i32> Map() => [\"a\" => 1, \"b\" => 2];\n\
              i32 Pairs(Dictionary<string, i32> d) {\n    let mut n = 0;\n    foreach (let kv in d) {\n        n += kv.Value;\n    }\n    return n;\n}\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    for bad in [
        "string F(List<i32> xs) => xs[0];\n",
        "void G(List<i32> xs) {\n    xs.Add(\"no\");\n}\n",
        "HashSet<i32> H() => [\"a\"];\n",
        "bool K(Dictionary<string, i32> d) => d.ContainsKey(3);\n",
        // A lambda typed by the delegate it's handed as: a `Predicate<i32>`'s.
        "void L(List<i32> xs) {\n    xs.RemoveAll(x => x + 1);\n}\n",
    ] {
        assert_eq!(codes(bad), [script_problem::TYPE_MISMATCH], "{bad}");
    }
    assert_eq!(
        codes("i32 C(List<i32> xs) => xs.Length;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
}

#[test]
fn strings_have_strings_members() {
    let ok = "string F(string s) => s.Trim().ToUpper().Substring(1, s.Length - 2);\n\
              char G(string s) => s[0];\n\
              bool H(string? s) => String.IsNullOrEmpty(s) || s!.StartsWith(\"a\");\n\
              string J(List<string> parts) => String.Join(\", \", in parts);\n\
              i32 K(string s) => s.Split(',').Length + s.IndexOf('x');\n\
              String L(string s) => s;\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    assert_eq!(
        codes("i32 F(string s) => s.Size;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
    assert_eq!(
        codes("i32 F(string s) => s.ToUpper();\n"),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn arrays_are_csharps() {
    let ok = "i32 Sum(i32[] xs) {\n    let mut total = 0;\n    foreach (let x in xs) {\n        total += x;\n    }\n    return total + xs.Length + xs[0];\n}\n\
              i32 Grid() {\n    let g = new i32[3, 4];\n    g[1, 2] = 5;\n    return g.GetLength(1) + g.Rank + g[1, 2];\n}\n\
              i32[][] Jagged() => new i32[3][];\n\
              string[] Names() => new[] { \"a\", \"b\" };\n\
              i32[] Made() => [1, 2, 3];\n\
              i32[,] Square() => new i32[,] { { 1, 2 }, { 3, 4 } };\n\
              void Sorted(i32[] xs) {\n    let mut ys = move xs;\n    Array.Sort(ref ys);\n    let i = Array.IndexOf(in ys, 3);\n}\n\
              i32 Count(in IEnumerable<i32> xs) => 0;\n\
              i32 Viewed(i32[] xs) => Count(in xs);\n\
              object[] Covariant(string[] names) => names;\n\
              string[] Parts(string s) => s.Split(',');\n\
              i32[] Back(List<i32> xs) => xs.ToArray();\n\
              void Grown(i32[] xs) {\n    let mut ys = move xs;\n    Array.Resize(ref ys, 10);\n}\n\
              i32[] None() => Array.Empty<i32>();\n\
              string[] Long(string[] names) => Array.FindAll(in names, n => n.Length > 2);\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    for bad in [
        "i32 F(i32[] xs) => xs[\"a\"];\n",
        "i64[] G(i32[] xs) => xs;\n",
        "i32[] H() => new i32[] { \"a\" };\n",
        "i32[,] K() => new i32[,] { 1, 2 };\n",
    ] {
        assert_eq!(codes(bad), [script_problem::TYPE_MISMATCH], "{bad}");
    }
    assert_eq!(
        codes("i32 F(i32[,] g) => g[1];\n"),
        [script_problem::WRONG_ARGS]
    );
    assert_eq!(
        codes("i32 F(i32[] xs) => xs.Count;\n"),
        [script_problem::UNKNOWN_MEMBER]
    );
}

#[test]
fn variance_is_csharps() {
    let ok = "IEnumerable<object> Out(List<string> xs) => xs;\n\
              IReadOnlyList<object> Read(List<string> xs) => xs;\n\
              Predicate<string> In(Predicate<object> p) => p;\n\
              Func<string, object> Both(Func<object, string> f) => f;\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    for bad in [
        // `List<T>` doesn't vary; a value type doesn't either.
        "List<object> F(List<string> xs) => xs;\n",
        "IEnumerable<object> G(List<i32> xs) => xs;\n",
        // `in` the other way only.
        "Predicate<object> H(Predicate<string> p) => p;\n",
    ] {
        assert_eq!(codes(bad), [script_problem::TYPE_MISMATCH], "{bad}");
    }
}

#[test]
fn every_type_is_an_object() {
    let ok = "class Tag(string name) {\n    public string Name = name;\n\
              public override string ToString() => Name;\n    public override i32 GetHashCode() => 7;\n}\n\
              struct Pos(i32 x) {\n    public i32 X = x;\n}\n\
              string F(Tag t, Pos p) => t.ToString() + p.ToString() + 5.ToString();\n\
              bool G(Pos a, Pos b) => a.Equals(b) && a.GetHashCode() == b.GetHashCode();\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    assert_eq!(
        codes("class Tag {\n    public override string Name() => \"\";\n}\n"),
        [script_problem::INHERIT]
    );
    // A class made implements its interfaces' members, the library's too.
    assert_eq!(
        codes("class Bag : IEnumerable<i32> {}\n"),
        [script_problem::INHERIT]
    );
}

#[test]
fn explicit_implementations_are_csharps() {
    let ok = "public interface IShape {\n    f64 Area();\n    string Name { get; }\n}\n\
              public class Square(f64 side) : IShape {\n    public f64 Side = side;\n\
              f64 IShape.Area() => Side * Side;\n    string IShape.Name => \"square\";\n}\n\
              f64 A(IShape s) => s.Area();\n\
              bool Added(HashSet<string> set) => set.Add(\"a\");\n\
              bool Pair(ICollection<KeyValuePair<string, i32>> pairs, Dictionary<string, i32> d) =>\n\
                  pairs.Remove(new KeyValuePair<string, i32>(\"a\", 1)) && d.Remove(\"a\");\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    let shape = "public interface IShape {\n    f64 Area();\n}\n";
    for (body, code) in [
        // Not one of its interfaces; one without the member; an access said.
        (
            "class A {\n    f64 IShape.Area() => 1;\n}\n",
            script_problem::INHERIT,
        ),
        (
            "class B : IShape {\n    f64 IShape.Area() => 1;\n    f64 IShape.Size() => 1;\n}\n",
            script_problem::INHERIT,
        ),
        (
            "class C : IShape {\n    public f64 IShape.Area() => 1;\n}\n",
            script_problem::INHERIT,
        ),
        // Reached through its interface only.
        (
            "class D : IShape {\n    f64 IShape.Area() => 1;\n}\nf64 F(D d) => d.Area();\n",
            script_problem::UNKNOWN_MEMBER,
        ),
    ] {
        assert_eq!(codes(&format!("{shape}{body}")), [code], "{body}");
    }
}

#[test]
fn linq_is_csharps() {
    let ok = "record Player(string Name, i32 Level);\n\
              List<string> High(List<Player> ps) => ps.Where(p => p.Level > 10).OrderBy(p => p.Name).Select(p => p.Name).ToList();\n\
              i32 Levels(List<Player> ps) => ps.Sum(p => p.Level) + ps.Count(p => p.Level > 3) + ps.Max(p => p.Level);\n\
              f64 Mean(i32[] xs) => xs.Average();\n\
              bool Any(i32[] xs) => xs.Any(x => x > 3) && xs.Contains(4);\n\
              Dictionary<string, i32> ByName(List<Player> ps) => ps.ToDictionary(p => p.Name, p => p.Level);\n\
              i32 Groups(List<Player> ps) {\n    let mut n = 0;\n    foreach (let g in ps.GroupBy(p => p.Level)) {\n        n += g.Key + g.Count();\n    }\n    return n;\n}\n\
              string? First(List<Player> ps) => ps.FirstOrDefault(p => p.Level > 99)?.Name;\n\
              i32 Squares() => Enumerable.Range(1, 10).Select(x => x * x).Where(x => x % 2 == 0).Sum();\n\
              i32 Total(List<i32> xs) => xs.Aggregate(0, (acc, x) => acc + x);\n\
              string[] Names(List<Player> ps) => ps.Select(p => p.Name).Distinct().ToArray();\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    for bad in [
        // `Select` gives strings: no `Level` on them; `Where` wants a `bool`.
        "record P(i32 Level);\ni32 F(List<P> ps) => ps.Select(p => p.Level).Sum(x => x.Level);\n",
        "i32 G(List<i32> xs) => xs.Where(x => x + 1).Count();\n",
    ] {
        assert!(!codes(bad).is_empty(), "{bad}");
    }
}

#[test]
fn the_other_collections_are_dotnets() {
    let ok = "i32 Q() {\n    let mut q = new Queue<i32>();\n    q.Enqueue(1);\n    let first = q.Dequeue();\n    return first + (q.TryPeek() ?? 0) + q.Count;\n}\n\
              string S() {\n    let mut s = new Stack<string>();\n    s.Push(\"a\");\n    return s.Pop();\n}\n\
              i32 Sorted() {\n    let mut set = new SortedSet<i32>();\n    let added = set.Add(3);\n    return set.Min + set.Max + set.GetViewBetween(1, 2).Count;\n}\n\
              string? Ranked(SortedDictionary<i32, string> d) => d.TryGetValue(1);\n\
              string Next() {\n    let mut pq = new PriorityQueue<string, i32>();\n    pq.Enqueue(\"low\", 5);\n    pq.Enqueue(\"high\", 1);\n    return pq.Dequeue();\n}\n\
              i32 Linked() {\n    let mut list = new LinkedList<i32>();\n    let node = list.AddLast(2);\n    list.AddBefore(node, 1);\n    return list.First!.Value + (list.Find(2)?.Next?.Value ?? 0);\n}\n\
              i32 Order(i32 a, i32 b) => Comparer<i32>.Default.Compare(a, b);\n\
              IComparer<string> ByLength() => Comparer<string>.Create((x, y) => x.Length - y.Length);\n\
              SortedSet<string> Named() => new SortedSet<string>(Comparer<string>.Create((x, y) => x.Length - y.Length));\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
    for bad in [
        "string F(Queue<i32> q) => q.Dequeue();\n",
        "void G(Stack<i32> s) {\n    s.Push(\"a\");\n}\n",
        "string H(PriorityQueue<string, i32> pq) {\n    pq.Enqueue(\"a\", \"b\");\n    return pq.Peek();\n}\n",
    ] {
        assert_eq!(codes(bad), [script_problem::TYPE_MISMATCH], "{bad}");
    }
}

#[test]
fn of_type_and_chunk_know_their_types() {
    let ok = "List<string> Texts(List<object> things) => things.OfType<string>().ToList();\n\
              i32 Firsts(List<i32> xs) {\n    let mut n = 0;\n    foreach (let c in xs.Chunk(3)) {\n        n += c[0] + c.Length;\n    }\n    return n;\n}\n\
              IEnumerable<string> All(List<object> things) => things.Cast<string>();\n";
    assert!(codes(ok).is_empty(), "{:?}", check("a.ys", ok).problems);
}
