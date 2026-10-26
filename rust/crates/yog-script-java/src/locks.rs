//! `lock (target, ref let value) body`, as C#'s `lock` over a data-owning
//! lock: entered, its value copied into `value` (its own local), `body`
//! run, the value written back and the lock let go of however `body` ends
//! — in `finally`.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, Name, Stmt};
use yog_script_ids::java::{FINALLY, TRY, VAR};

use crate::Gen;

use yog_script_ids::members::{DISPOSE, ENTER, VALUE};

impl Gen<'_> {
    pub(crate) fn lock_stmt(&mut self, target: &Expr, value: Option<&Name>, body: &Stmt) {
        let lock = self.expr(target);
        let guard = self.temp();
        self.code.line(format!("{VAR} {guard} = {lock}.{ENTER}();"));
        let held = match self.types.of(target).map(Type::non_null) {
            Some(Type::Own(_, args)) if args.len() == 1 => args[0].clone(),
            _ => Type::Unknown,
        };
        if let Some(n) = value {
            let t = self.java(&held, false);
            self.code
                .line(format!("{t} {} = {guard}.{VALUE}();", Self::ident(&n.text)));
        }
        self.code.open(TRY);
        match body {
            Stmt::Block(b) => self.block_stmts(b),
            other => self.stmt(other),
        }
        self.code.close("");
        self.code.open(FINALLY);
        if let Some(n) = value {
            self.code.line(format!(
                "{guard}.{}({});",
                crate::stmts::setter(VALUE),
                Self::ident(&n.text)
            ));
        }
        self.code.line(format!("{guard}.{DISPOSE}();"));
        self.code.close("");
    }
}
