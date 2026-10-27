//! Declared state (`saved`, `synced`; docs/design/storage.md S3): Yog
//! Data, on an owner.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("A.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn saved_state_is_yog_data_on_an_owner() {
    let fine = "using System.Collections.Generic;\n\
                public record Spot(i32 X, i32 Y);\n\
                public enum Mode { Idle, Charging(i32 Rate) }\n\
                extend Player {\n\
                    public saved i32 Visits = 0;\n\
                    public saved List<Spot> Spots = new List<Spot>();\n\
                    public saved Mode Mode = Mode.Idle;\n\
                    public saved Player? Friend = null;\n\
                    public saved ItemId Favourite = new ItemId(\"stick\");\n\
                }\n";
    assert!(codes(fine).is_empty(), "{:?}", check("A.ys", fine).problems);
    let not_data = "using System;\nextend World {\n    public saved Func<i32> Later = () => 1;\n}\n";
    assert_eq!(codes(not_data), [script_problem::STATE_TYPE]);
    let no_owner = "public class Plain {\n    public saved i32 Count = 0;\n}\n";
    assert_eq!(codes(no_owner), [script_problem::STATE_OWNER]);
}
