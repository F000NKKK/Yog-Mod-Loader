use super::*;
use yog_script_analyze::Env;
use yog_script_analyze::ast::VersionCond;
use yog_script_analyze::index::{Class, Member, Side, access};

fn cond(src: &str) -> VersionCond {
    let parsed = yog_script_analyze::parse("a.yns", src);
    yog_script_analyze::targets(&parsed.file).unwrap().clone()
}

#[test]
fn targets_take_the_versions_yog_runs_on() {
    assert_eq!(
        targeted(&cond("targets mc 1.20.1..26.2;\n")),
        game_version::ALL_VALUES
    );
    assert_eq!(
        targeted(&cond("targets mc >= 1.21;\n")),
        [game_version::V1_21_1, game_version::V26_2]
    );
    assert_eq!(
        targeted(&cond("targets mc == 1.20.1;\n")),
        [game_version::V1_20_1]
    );
    assert!(targeted(&cond("targets mc < 1.19;\n")).is_empty());
}

/// A game whose `LivingEntity` has `getHealth` on some versions only.
fn game(mc: &str) -> Result<ClassIndex, String> {
    let health = Member {
        name: "getHealth".into(),
        desc: "()F".into(),
        access: access::PUBLIC,
    };
    let methods = if mc == game_version::V26_2 {
        vec![]
    } else {
        vec![health]
    };
    Ok(ClassIndex {
        mc: mc.into(),
        classes: vec![Class {
            name: "net/minecraft/world/entity/LivingEntity".into(),
            side: Side::Shared,
            access: access::PUBLIC,
            extends: None,
            implements: vec![],
            fields: vec![],
            methods,
        }],
    })
}

#[test]
fn a_patch_is_checked_on_each_version() {
    let src = "targets mc 1.20.1..26.2;\nusing System.Game.World.Entity;\n\npatch LivingEntity {\n    after float GetMaxHealth() {\n        float h = this.Health;\n    }\n}\n";
    let problems = check("a.yns", src, game, Env::default());
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert!(
        problems[0].message.ends_with("(on 26.2)"),
        "{}",
        problems[0].message
    );
    let none = check("a.yns", "targets mc < 1.19;\n", game, Env::default());
    assert_eq!(none[0].code.as_deref(), Some(script_problem::NO_VERSION));
}
