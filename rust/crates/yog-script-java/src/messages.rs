//! Calls across sides (`ys.md` §4.1): the file's `client` and `server`
//! methods each a message — a record of its arguments (`ShowChargeMessage`;
//! a `server` one's `Sender`, who sent it, not among them) on its channel
//! (`charge:main/show_charge`), run when it arrives (registered by
//! `$Load`); a call the checker found crossing sides sends one.

use yog_script_analyze::Message;
use yog_script_analyze::ast::{Arg, Decl, Expr, ExprKind, Method, Modifier, TypeKind};
use yog_script_ids::java::{CLASS_LITERAL, NEW, PUBLIC, RECORD};
use yog_script_ids::jvm::{
    MESSAGE_SUFFIX, MESSAGES_ON_CLIENT, MESSAGES_ON_SERVER, MESSAGES_TO_CLIENT, MESSAGES_TO_SERVER,
};
use yog_script_ids::system::SENDER;

use crate::Gen;

impl Gen<'_> {
    /// The channel `method`'s messages go on: the mod's, the source's and
    /// the method's; none in a source of no mod.
    fn channel(&self, method: &str) -> Option<String> {
        let mod_id = self.mod_id.as_deref()?;
        let stem = std::path::Path::new(&self.file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let path = format!(
            "{}{}{}",
            yog_codegen::snake(stem),
            yog_ids::sep::PATH,
            yog_codegen::snake(method)
        );
        Some(self.code.str(&yog_ids::join(mod_id, &path)))
    }

    /// The file's method `m` (`d`), when it's `client` or `server`: its
    /// message's record, its runs when one arrives registered.
    pub(crate) fn message_endpoint(&mut self, d: &Decl, m: &Method) {
        let to_server = d.has(Modifier::Server);
        if !to_server && !d.has(Modifier::Client) {
            return;
        }
        let Some(channel) = self.channel(&m.name.text) else {
            return;
        };
        let takes_sender = to_server && m.params.first().is_some_and(|p| is_sender(&p.ty));
        let carried = if takes_sender {
            &m.params[1..]
        } else {
            &m.params[..]
        };
        let record = format!("{}{MESSAGE_SUFFIX}", m.name.text);
        let fields = self.params(carried);
        self.code
            .line(format!("{PUBLIC} {RECORD} {record}({fields}) {{}}"));
        let (sent, from) = (self.temp(), self.temp());
        let mut args: Vec<String> = carried
            .iter()
            .map(|p| format!("{sent}.{}()", Self::ident(&p.name.text)))
            .collect();
        if takes_sender {
            args.insert(0, from.clone());
        }
        let owner = self.class_name.clone();
        let call = format!("{owner}.{}({})", Self::ident(&m.name.text), args.join(", "));
        let class = format!("{owner}.{record}{CLASS_LITERAL}");
        let line = if to_server {
            format!("{MESSAGES_ON_SERVER}({channel}, {class}, ({from}, {sent}) -> {call});")
        } else {
            format!("{MESSAGES_ON_CLIENT}({channel}, {class}, {sent} -> {call});")
        };
        self.on_load.push(line);
    }

    /// Call `e` (of `callee` with `args`) across sides: its message sent.
    pub(crate) fn message_sent(
        &mut self,
        e: &Expr,
        (callee, args): (&Expr, &[Arg]),
        m: &Message,
    ) -> String {
        let Some(channel) = self.channel(&m.method) else {
            self.not_yet("a call across sides in a source of no mod", e.span);
            return String::new();
        };
        let record = format!("{}.{}{MESSAGE_SUFFIX}", self.class_name, m.method);
        let made = format!("{NEW} {record}({})", self.args(args));
        if m.to_server {
            return format!("{MESSAGES_TO_SERVER}({channel}, {made})");
        }
        // `player.Client.F(…)`: the player's client.
        let target = match &callee.kind {
            ExprKind::Member { expr, .. } => match &expr.kind {
                ExprKind::Member { expr: target, .. } => self.expr(target),
                _ => String::new(),
            },
            _ => String::new(),
        };
        format!("{MESSAGES_TO_CLIENT}({target}, {channel}, {made})")
    }
}

/// Whether written type `t` is who sent a message (`Sender`).
fn is_sender(t: &yog_script_analyze::ast::Type) -> bool {
    matches!(&t.kind, TypeKind::Path { path, args } if args.is_empty() && path.last().is_some_and(|n| n.text == SENDER))
}
