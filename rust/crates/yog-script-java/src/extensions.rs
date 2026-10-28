//! `extend T { … }` on the JVM: its members a class's of their own
//! (`$ExtendPlayer…`), one instance per owner — a player's, an entity's;
//! the world's one — reached by `$of(owner)`, the owner's handle kept as
//! `$owner` (what `this` is in its code). `p.Visits` is
//! `$ExtendPlayer….$of(p).Visits`.

use std::collections::HashSet;

use yog_script_analyze::ast::{Decl, DeclKind, Type as Written, TypeHeader, TypeKind};
use yog_script_ids::java::{
    CLASS, FINAL, IMPLEMENTS, METHOD_REF, NEW, PRIVATE, PUBLIC, RETURN, STATIC, THIS,
};
use yog_script_ids::jvm::extend::{ALL, CLASS_PREFIX, FIND_OR_MAKE, INSTANCES, OF, OWNER};
use yog_script_ids::jvm::state::{CLASS as STATE, PLAYER, SAVED, WORLD};
use yog_script_ids::jvm::synced::{
    CLASS as SYNCED_STATE, FORGET_SYNCED, OWNERS, SYNCED_INTERFACE, TO_ALL, TO_PLAYER,
};
use yog_script_ids::jvm::{MAP, STRING};

use super::Gen;
use crate::decls::Place;
use crate::index::{Index, Info, Kind};

/// An `extend` block's class: whose members it adds, where it is.
pub(crate) struct Extension {
    owner: String,
    class: String,
    place: String,
    members: HashSet<String>,
}

/// The owner type `ty` names, and its block's class's name (its own: one
/// a block, by where it's written).
fn class_of(ty: &Written) -> Option<(String, String)> {
    let TypeKind::Path { path, .. } = &ty.kind else {
        return None;
    };
    let owner = path.last()?.text.clone();
    let class = format!("{CLASS_PREFIX}{owner}{}", ty.span.start);
    Some((owner, class))
}

/// The members `body` adds.
fn member_names(body: &[Decl]) -> HashSet<String> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Field(f) => Some(f.name.text.clone()),
            DeclKind::Property { name, .. } => Some(name.text.clone()),
            DeclKind::Method(m) => Some(m.name.text.clone()),
            _ => None,
        })
        .collect()
}

impl Index {
    /// `extend ty { body }` (`header` its own) walked: its class a type of
    /// the source's, its members noted by their owner.
    pub(crate) fn note_extension(
        &mut self,
        ty: &Written,
        header: &TypeHeader,
        body: &[Decl],
        elsewhere: Option<&str>,
    ) {
        let Some((owner, class)) = class_of(ty) else {
            return;
        };
        let key = (class.clone(), 0);
        self.declared_type(&class);
        self.types.insert(
            key.clone(),
            Info {
                kind: Kind::Class,
                computed: crate::index::computed(body),
                bases: Vec::new(),
                takes_args: true,
                is_abstract: false,
                record_params: Vec::new(),
                record_fields: Vec::new(),
            },
        );
        self.note_shape(key, header, body);
        let place = match elsewhere {
            Some(file) => format!("{file}.{class}"),
            None => class.clone(),
        };
        self.extensions.push(Extension {
            owner,
            class,
            place,
            members: member_names(body),
        });
    }

    /// The class of the `extend` adding member `name` to `owner`, if one
    /// does: its name, and where it is.
    pub(crate) fn extension(&self, owner: &str, name: &str) -> Option<(&str, &str)> {
        self.extensions
            .iter()
            .find(|x| x.owner == owner && x.members.contains(name))
            .map(|x| (x.class.as_str(), x.place.as_str()))
    }
}

impl Gen<'_> {
    /// `extend ty { body }`'s class: its owners' instances, its members.
    pub(crate) fn extend_decl(&mut self, ty: &Written, body: &[Decl]) {
        let Some((owner, class)) = class_of(ty) else {
            return self.not_yet("an `extend` of this type", ty.span);
        };
        // The world's state is one: every dimension's handle its owner.
        let key = if owner == yog_script_analyze::yog::world() {
            self.code.str("")
        } else {
            OWNER.to_string()
        };
        let saved = !crate::saved::saved_members(body).is_empty();
        let synced = !crate::saved::synced_members(body).is_empty();
        // What a hot reload keeps of an owner's instance: its members that aren't saved.
        let session = !crate::session::session_members(body).is_empty();
        let interfaces: Vec<String> = [
            (saved, format!("{STATE}.{SAVED}")),
            (synced, format!("{SYNCED_STATE}.{SYNCED_INTERFACE}")),
            (session, crate::session::kept_interface()),
        ]
        .into_iter()
        .filter_map(|(has, i)| has.then_some(i))
        .collect();
        let implements = if interfaces.is_empty() {
            String::new()
        } else {
            format!(" {IMPLEMENTS} {}", interfaces.join(", "))
        };
        self.code.open(format!(
            "{PUBLIC} {STATIC} {FINAL} {CLASS} {class}{implements}"
        ));
        self.code.line(format!(
            "{PRIVATE} {STATIC} {FINAL} {MAP}<{STRING}, {class}> {ALL} = {NEW} {INSTANCES}<>();"
        ));
        self.code
            .line(format!("{PRIVATE} {FINAL} {STRING} {OWNER};"));
        for (t, n) in crate::classes::fields(body) {
            let t = self.written(t, false);
            self.code.line(format!("{PUBLIC} {t} {};", Self::ident(n)));
        }
        let outer = (
            self.owner.replace((class.clone(), 0)),
            std::mem::replace(&mut self.extending, true),
        );
        self.code
            .open(format!("{PRIVATE} {class}({STRING} {OWNER})"));
        self.code.line(format!("{THIS}.{OWNER} = {OWNER};"));
        self.first_values(body);
        self.code.close("");
        self.code
            .open(format!("{PUBLIC} {STATIC} {class} {OF}({STRING} {OWNER})"));
        let made = self.live_made(&owner, &class, ty, (saved, synced, session));
        self.code
            .line(format!("{RETURN} {ALL}.{FIND_OR_MAKE}({key}, {made});"));
        self.code.close("");
        if saved || synced {
            let mut forget = format!("{ALL}.remove({THIS}.{OWNER});");
            if synced {
                forget.push_str(&format!(" {SYNCED_STATE}.{FORGET_SYNCED}({THIS});"));
                // A client's object of an owner: what its rows go to.
                let kind = self.code.str(&self.synced_kind(&class));
                self.on_load.push(format!(
                    "{SYNCED_STATE}.{OWNERS}({kind}, {class}{METHOD_REF}{OF});"
                ));
            }
            self.saved_state(&owner, body, (&forget, false));
        }
        if session {
            self.session_state(body);
        }
        self.members(body, Place::Class);
        (self.owner, self.extending) = outer;
        self.code.close("");
    }

    /// What makes an owner's instance of `class` (an `extend` of `owner`):
    /// made; read in from its scope's store when it keeps `saved` members;
    /// live to its clients when it keeps `synced` ones (a player's to that
    /// player, the world's to every one); live in the session (`session`)
    /// when a hot reload keeps some of its members.
    fn live_made(
        &mut self,
        owner: &str,
        class: &str,
        ty: &Written,
        (saved, synced, session): (bool, bool, bool),
    ) -> String {
        if !(saved || synced || session) {
            return format!("{class}{METHOD_REF}{NEW}");
        }
        let module = self.code.str(self.mod_id.as_deref().unwrap_or_default());
        let key = self.temp();
        if !(saved || synced) {
            let made = format!("{NEW} {class}({key})");
            return format!(
                "{key} -> {}",
                crate::session::live(&made, (&module, class, &key))
            );
        }
        let world = owner == yog_script_analyze::yog::world();
        if !world && owner != yog_script_analyze::yog::player() {
            self.not_yet("`saved` or `synced` members of this object", ty.span);
            return format!("{class}{METHOD_REF}{NEW}");
        }
        let mut made = format!("{NEW} {class}({key})");
        if saved {
            made = if world {
                format!("{STATE}.{WORLD}({made}, {module})")
            } else {
                format!("{STATE}.{PLAYER}({made}, {module}, {key})")
            };
        }
        if synced {
            let kind = self.code.str(&self.synced_kind(class));
            made = if world {
                format!("{SYNCED_STATE}.{TO_ALL}({made}, {kind}, {key})")
            } else {
                format!("{SYNCED_STATE}.{TO_PLAYER}({made}, {kind}, {key})")
            };
        }
        if session {
            made = crate::session::live(&made, (&module, class, &key));
        }
        format!("{key} -> {made}")
    }

    /// What a class's synced rows say they're of: the mod's id and the class.
    pub(crate) fn synced_kind(&self, class: &str) -> String {
        format!(
            "{}{}{class}",
            self.mod_id.as_deref().unwrap_or_default(),
            yog_ids::sep::NAMESPACE
        )
    }

    /// Member `name` of `target` (a handle of `owner`) when an `extend`
    /// adds it: its owner's instance's.
    pub(crate) fn extension_member(&self, owner: &str, name: &str, target: &str) -> Option<String> {
        let (_, place) = self.index.extension(owner, name)?;
        Some(format!("{place}.{OF}({target})"))
    }

    /// Member `name` of `target` (a handle of `owner`) read, when an
    /// `extend` adds it: its field, or its property's getter.
    pub(crate) fn extension_read(&self, owner: &str, name: &str, target: &str) -> Option<String> {
        let (class, _) = self.index.extension(owner, name)?;
        let of = self.extension_member(owner, name, target)?;
        let member = Self::ident(name);
        Some(if self.index.called(&(class.to_string(), 0), name) {
            format!("{of}.{member}()")
        } else {
            format!("{of}.{member}")
        })
    }
}
