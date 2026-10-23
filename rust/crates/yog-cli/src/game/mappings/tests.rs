//! The formats read and joined, on samples of each.

use super::{join, proguard, runtime};

const MOJANG: &str = concat!(
    "# comment\n",
    "net.minecraft.world.entity.LivingEntity -> bfz:\n",
    "    float health -> a\n",
    "    169:169:boolean hurt(net.minecraft.world.damagesource.DamageSource,float) -> a\n",
    "    void <init>() -> <init>\n",
    "net.minecraft.world.damagesource.DamageSource -> bfa:\n",
);

const TINY: &str = concat!(
    "tiny\t2\t0\tofficial\tintermediary\n",
    "c\tbfz\tnet/minecraft/class_1309\n",
    "\tf\tF\ta\tfield_6213\n",
    "\tm\t(Lbfa;F)Z\ta\tmethod_5643\n",
    "c\tbfa\tnet/minecraft/class_1282\n",
);

const TSRG: &str = concat!(
    "tsrg2 obf srg id\n",
    "bfz net/minecraft/src/C_1_ 1\n",
    "\ta f_20958_ 2\n",
    "\ta (Lbfa;F)Z m_6469_ 3\n",
    "\t\t0 o p_1\n",
    "bfa net/minecraft/src/C_2_ 4\n",
);

#[test]
fn a_proguard_method_gets_its_descriptor() {
    assert_eq!(proguard::type_desc("int[][]"), "[[I");
    let classes = proguard::parse(MOJANG);
    let living = &classes[0];
    assert_eq!(
        (living.named.as_str(), living.obf.as_str()),
        ("net/minecraft/world/entity/LivingEntity", "bfz")
    );
    assert_eq!(
        living.methods[0].0.desc,
        "(Lnet/minecraft/world/damagesource/DamageSource;F)Z"
    );
    let obf = proguard::remap_desc(&living.methods[0].0.desc, &proguard::class_map(&classes));
    assert_eq!(obf, "(Lbfa;F)Z");
}

#[test]
fn intermediary_renames_classes_and_members() {
    let table = join(
        &proguard::parse(MOJANG),
        &runtime::tiny(TINY).unwrap(),
        "intermediary",
        false,
    );
    let living = table
        .classes
        .iter()
        .find(|c| c.named.ends_with("LivingEntity"))
        .unwrap();
    assert_eq!(living.runtime, "net/minecraft/class_1309");
    assert_eq!(living.fields[0].runtime, "field_6213");
    assert_eq!(
        (
            living.methods[0].named.as_str(),
            living.methods[0].runtime.as_str()
        ),
        ("hurt", "method_5643")
    );
    assert_eq!(
        living.methods.len(),
        1,
        "a constructor keeps its name: left out"
    );
}

#[test]
fn srg_keeps_mojangs_class_names() {
    let table = join(&proguard::parse(MOJANG), &runtime::tsrg(TSRG), "srg", true);
    let living = table
        .classes
        .iter()
        .find(|c| c.named.ends_with("LivingEntity"))
        .unwrap();
    assert_eq!(living.runtime, living.named);
    assert_eq!(
        (
            living.fields[0].runtime.as_str(),
            living.methods[0].runtime.as_str()
        ),
        ("f_20958_", "m_6469_")
    );
    assert!(
        table
            .classes
            .iter()
            .all(|c| !c.named.ends_with("DamageSource")),
        "nothing renamed: left out"
    );
}
