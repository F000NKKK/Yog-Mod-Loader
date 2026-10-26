//! `where` constraints: a type parameter's members are its bounds', and a
//! type given for it must meet them (C#'s CS0311, CS0452, CS0453, CS0310, CS0304).

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("a.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

const DECLS: &str = "public interface IScore { i32 Score(); }\n\
                     public class Card : IScore { public Card(i32 n) {} public i32 Score() => 1; }\n\
                     public struct Point { public i32 X; }\n\
                     public static class G {\n\
                         public static i32 Of<TItem>(TItem t) where TItem : IScore => t.Score();\n\
                         public static TItem Ref<TItem>(TItem t) where TItem : class => t;\n\
                         public static TItem Val<TItem>(TItem t) where TItem : struct => t;\n\
                         public static TItem Made<TItem>() where TItem : new() => new TItem();\n\
                     }\n";

#[test]
fn bounds_give_members() {
    assert_eq!(codes(DECLS), Vec::<String>::new());
    let unbounded =
        "public static class H { public static i32 Of<TItem>(TItem t) => t.Score(); }\n";
    assert_eq!(codes(unbounded), [script_problem::UNKNOWN_MEMBER]);
    let made = "public static class H { public static TItem Made<TItem>() => new TItem(); }\n";
    assert_eq!(codes(made), [script_problem::CONSTRAINT]);
}

#[test]
fn arguments_meet_constraints() {
    let ok = "public static class U { public static void Run() {\n\
                  G.Of(new Card(1)); G.Ref(\"s\"); G.Val(new Point()); G.Made<Point>(); G.Made<i32>();\n\
              } }\n";
    assert_eq!(codes(&format!("{DECLS}{ok}")), Vec::<String>::new());
    let bad = "public static class U { public static void Run() {\n\
                   G.Of(5); G.Ref(5); G.Val(\"s\"); G.Made<Card>();\n\
               } }\n";
    assert_eq!(
        codes(&format!("{DECLS}{bad}")),
        vec![script_problem::CONSTRAINT; 4]
    );
}

#[test]
fn written_types_meet_their_types_constraints() {
    let src = "public interface IScore { i32 Score(); }\n\
               public class Pile<TItem> where TItem : IScore { }\n\
               public static class U { public static void Run(Pile<string> p) { } }\n";
    assert_eq!(codes(src), [script_problem::CONSTRAINT]);
}
