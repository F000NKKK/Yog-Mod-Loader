//! Families of key spaces: one member a dimension (`yog:keys/block/<dim>`:
//! a block's key, its packed position, fills all 64 bits, so the dimension
//! is the space's). A member is made the first time it's named; a
//! family's templates (the host's block mirror) are provided in each
//! member as it's made — `<template>/<member's parameter>`.

use yog_data::Diagnostic;
use yog_ids::sep;

use super::{SpaceId, Store, unknown_space};
use crate::spec::MirrorSpec;

/// A mirror every member of a family has, and who writes it.
#[derive(Clone)]
pub(super) struct Template {
    spec: MirrorSpec,
    writer: u32,
    host: bool,
}

/// A family: its name (`yog:keys/block`) and templates.
pub(super) struct Family {
    name: String,
    templates: Vec<Template>,
}

/// `member`'s parameter in family `family` (`minecraft:overworld`), if it's one.
fn parameter<'a>(family: &str, member: &'a str) -> Option<&'a str> {
    member
        .strip_prefix(family)?
        .strip_prefix(sep::PATH)
        .filter(|p| !p.is_empty())
}

impl Store {
    /// Makes `name` a family: its members are made when they're named.
    pub fn family(&mut self, name: &str) {
        if !self.families.iter().any(|f| f.name == name) {
            self.families.push(Family {
                name: name.to_owned(),
                templates: Vec::new(),
            });
        }
    }

    /// Whether `name` is a family.
    pub fn is_family(&self, name: &str) -> bool {
        self.families.iter().any(|f| f.name == name)
    }

    /// Whether space `member` is of family `family`.
    pub(super) fn member_of(family: &str, member: &str) -> bool {
        parameter(family, member).is_some()
    }

    /// Makes `writer` write `template` (its space a family) in every member:
    /// those there are now and those made later.
    pub fn provide_family(
        &mut self,
        template: &MirrorSpec,
        writer: u32,
        host: bool,
    ) -> Result<(), Diagnostic> {
        let Some(at) = self.families.iter().position(|f| f.name == template.space) else {
            return Err(unknown_space(&template.space));
        };
        let t = Template {
            spec: template.clone(),
            writer,
            host,
        };
        self.families[at].templates.push(t.clone());
        let members: Vec<String> = self
            .space_ids
            .keys()
            .filter(|m| Self::member_of(&template.space, m))
            .cloned()
            .collect();
        for m in members {
            self.instantiate(&t, &m)?;
        }
        Ok(())
    }

    /// Space `name`: made if it's a family's member not made yet.
    pub fn ensure_space(&mut self, name: &str) -> Result<SpaceId, Diagnostic> {
        if let Some(id) = self.space_id(name) {
            return Ok(id);
        }
        let Some(family) = self
            .families
            .iter()
            .find(|f| Self::member_of(&f.name, name))
        else {
            return Err(unknown_space(name));
        };
        let templates = family.templates.clone();
        let id = self.space(name, None)?;
        for t in &templates {
            self.instantiate(t, name)?;
        }
        Ok(id)
    }

    /// `t` provided in member `member`.
    fn instantiate(&mut self, t: &Template, member: &str) -> Result<(), Diagnostic> {
        let param = parameter(&t.spec.space, member).unwrap_or_default();
        let spec = MirrorSpec {
            name: format!("{}{}{param}", t.spec.name, sep::PATH),
            space: member.to_owned(),
            fields: t.spec.fields.clone(),
            clock: t.spec.clock,
        };
        self.provide(&spec, t.writer, t.host)
    }
}
