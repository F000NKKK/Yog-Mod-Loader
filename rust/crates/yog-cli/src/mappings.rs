//! `yog mappings` (`docs/design/nanites.md`, "Names"): a host's mapping
//! table — Mojang's names → the host's run-time names — from Mojang's
//! mappings and the loader's (Fabric's intermediary, Forge's SRG), fetched
//! once into Yog's cache.

mod fetch;
mod proguard;
mod runtime;
#[cfg(test)]
mod tests;

use yog_event::nanite::{ClassMap, Mappings, MemberMap};
use yog_ids::mapping_ns;

/// Mojang's classes joined with `rt`: each class and member by Mojang's
/// name, renamed as `rt` names it (`keep_classes`: Forge keeps Mojang's
/// class names at run time, renaming members only).
pub(crate) fn join(
    mojang: &[proguard::Class],
    rt: &runtime::Runtime,
    runtime_ns: &str,
    keep_classes: bool,
) -> Mappings {
    let obf_of = proguard::class_map(mojang);
    let classes = mojang
        .iter()
        .map(|c| {
            let renamed = rt.classes.get(&c.obf).filter(|_| !keep_classes);
            let field = |(n, obf): &(proguard::Named, String)| MemberMap {
                named: n.name.clone(),
                desc: String::new(),
                runtime: rt
                    .fields
                    .get(&(c.obf.clone(), obf.clone()))
                    .cloned()
                    .unwrap_or_else(|| n.name.clone()),
            };
            let method = |(n, obf): &(proguard::Named, String)| {
                let obf_desc = proguard::remap_desc(&n.desc, &obf_of);
                MemberMap {
                    named: n.name.clone(),
                    desc: n.desc.clone(),
                    runtime: rt
                        .methods
                        .get(&(c.obf.clone(), obf.clone(), obf_desc))
                        .cloned()
                        .unwrap_or_else(|| n.name.clone()),
                }
            };
            ClassMap {
                named: c.named.clone(),
                runtime: renamed.cloned().unwrap_or_else(|| c.named.clone()),
                fields: c.fields.iter().map(field).collect(),
                methods: c.methods.iter().map(method).collect(),
            }
        })
        .collect();
    let mut out = Mappings {
        runtime: runtime_ns.to_owned(),
        classes,
    };
    out.prune();
    out
}

/// The table for Minecraft `mc` on a host running `runtime_ns` names.
pub fn table(mc: &str, runtime_ns: &str) -> Result<Mappings, String> {
    if runtime_ns == mapping_ns::MOJANG {
        return Ok(Mappings {
            runtime: runtime_ns.to_owned(),
            classes: Vec::new(),
        });
    }
    let mojang = proguard::parse(&fetch::mojang(mc)?);
    match runtime_ns {
        mapping_ns::INTERMEDIARY => {
            let rt = runtime::tiny(&fetch::intermediary(mc)?)?;
            Ok(join(&mojang, &rt, runtime_ns, false))
        }
        mapping_ns::SRG => Ok(join(
            &mojang,
            &runtime::tsrg(&fetch::srg(mc)?),
            runtime_ns,
            true,
        )),
        other => Err(format!(
            "no run-time names `{other}`: {}, {} or {}",
            mapping_ns::MOJANG,
            mapping_ns::INTERMEDIARY,
            mapping_ns::SRG
        )),
    }
}
