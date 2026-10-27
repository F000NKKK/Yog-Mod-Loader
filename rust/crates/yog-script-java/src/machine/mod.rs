//! `async` methods: a machine of states, as C#'s compiler makes one. The
//! method makes a local class of the runtime's `Async<T>` and starts it;
//! the class's `$step` is the method's code in a `$loop` of a `switch` on
//! its `$state`, each `await` a state it goes on from once its task is
//! done. Its parameters, and the locals of the code an `await` splits,
//! are its fields (they live across a wait); the code no `await` splits
//! is written as any (`flow.rs`), an `await` in a value as it's met
//! (`awaits.rs`).

mod async_foreach;
mod awaits;
mod branches;
mod flow;
mod switches;

use yog_script_analyze::Type;
use yog_script_analyze::ast::{
    Expr, IT, LambdaBody, LambdaParam, Method, MethodBody, Pass, Stmt, TypeKind,
};
use yog_script_ids::java::{
    CASE, CLASS, CONTINUE, DEFAULT, EXTENDS, FINAL, IF, NEW, NULL, OVERRIDE, PROTECTED, RETURN,
    SUPER, SWITCH, THIS, TRUE, VAR, VOID, WHILE,
};
use yog_script_ids::jvm::OBJECT;
use yog_script_ids::jvm::tasks::{
    ASYNC, ASYNC_ITERATION, ASYNC_ITERATION_SOURCE, FINISH, ITERATION, ITERATION_SOURCE, LOOP,
    MACHINE, MADE, PARAM_FIELD, RUN, START, STATE, STEP, TASK, YIELD,
};
use yog_script_ids::system::{ASYNC_ENUMERATOR, ENUMERATOR};

use super::Gen;

pub(crate) use awaits::{awaits, stmt_awaits};

/// The `async` method being written's machine.
#[derive(Default)]
pub(crate) struct Machine {
    /// Its fields, each declared (`int x$v1;`).
    pub fields: Vec<String>,
    /// The states it has so far; the first, `0`, where it starts.
    pub states: usize,
    /// Its task's `T`, boxed: what a `try`'s failure finishes it as.
    pub gives: String,
}

/// What a machine is made of: its parameters (each its name in the source
/// and Java's type), its code, what it finishes with.
pub(crate) struct Shape<'a> {
    pub params: Vec<(String, String)>,
    pub body: Body<'a>,
    /// Its task's `T`, boxed (`Object` for a `Task`'s nothing).
    pub gives: String,
    /// It gives a task: a `Task<T>`'s value (`gives_value`) or none's; else
    /// it's an `async void`, started, its panic logged.
    pub task: bool,
    pub gives_value: bool,
    /// An iterator's: `gives` its items' type, each `yield return` one.
    pub iterator: bool,
    /// An `async` iterator's (`IAsyncEnumerable<T>`): awaits between its items.
    pub async_iterator: bool,
    /// An iterator giving its enumerator (`IEnumerator<T>`), not a sequence:
    /// the machine itself, walked once.
    pub enumerator: bool,
}

/// An async method's or lambda's code.
pub(crate) enum Body<'a> {
    Block(&'a [Stmt]),
    Expr(&'a Expr),
}

impl Gen<'_> {
    /// Async method `m` (Java's `head`): a machine made, started, its task
    /// given (an `async void`'s nothing: its panic is logged).
    pub(crate) fn async_method(&mut self, head: &str, m: &Method) {
        if m.params
            .iter()
            .any(|p| matches!(p.pass, Some(Pass::Ref | Pass::Out)))
        {
            return self.not_yet("an `async` method taking `ref` or `out`", m.name.span);
        }
        let body = match &m.body {
            Some(MethodBody::Block(b)) => Body::Block(&b.stmts),
            Some(MethodBody::Expr(e)) => Body::Expr(e),
            None => return self.not_yet("an `async` method without a body", m.name.span),
        };
        let task_arg = match m.ret.as_ref().map(|t| &t.kind) {
            Some(TypeKind::Path { args, .. }) => args.first(),
            _ => None,
        };
        let shape = Shape {
            params: m
                .params
                .iter()
                .map(|p| (p.name.text.clone(), self.written(&p.ty, false)))
                .collect(),
            body,
            gives: task_arg.map_or_else(|| OBJECT.to_string(), |t| self.written(t, true)),
            task: m.ret.is_some(),
            gives_value: task_arg.is_some(),
            iterator: false,
            async_iterator: false,
            enumerator: false,
        };
        self.code.open(head);
        self.machine_of(&shape);
        self.code.close("");
    }

    /// Iterator method `m` (Java's `head`; `async`: an `IAsyncEnumerable`'s,
    /// awaiting between its items): a sequence making its machine for each
    /// walk, each `yield return` an item it gives.
    pub(crate) fn iterator_method(&mut self, head: &str, m: &Method, is_async: bool) {
        let Some(MethodBody::Block(b)) = &m.body else {
            return self.not_yet("an iterator without a block", m.name.span);
        };
        let (item, enumerator) = match m.ret.as_ref().map(|t| &t.kind) {
            Some(TypeKind::Path { args, path }) => (
                args.first(),
                path.last()
                    .is_some_and(|n| [ENUMERATOR, ASYNC_ENUMERATOR].contains(&n.text.as_str())),
            ),
            _ => (None, false),
        };
        let shape = Shape {
            params: m
                .params
                .iter()
                .map(|p| (p.name.text.clone(), self.written(&p.ty, false)))
                .collect(),
            body: Body::Block(&b.stmts),
            gives: item.map_or_else(|| OBJECT.to_string(), |t| self.written(t, true)),
            task: false,
            gives_value: false,
            iterator: true,
            async_iterator: is_async,
            enumerator,
        };
        self.code.open(head);
        self.machine_of(&shape);
        self.code.close("");
    }

    /// `yield return v;` in an iterator's machine: `v` given, the machine
    /// leaving until the next item is asked for.
    pub(crate) fn yield_item(&mut self, v: &Expr) {
        let v = self.handed_expr(v);
        let state = self.new_state();
        self.code.line(format!("{YIELD}({v});"));
        self.code.line(format!("{STATE} = {state};"));
        self.code.line(format!("{RETURN};"));
        self.state_starts(state);
    }

    /// An `async` lambda (`e`, of a delegate's type): Java's lambda making
    /// its machine, as a method's.
    pub(crate) fn async_lambda(
        &mut self,
        e: &Expr,
        params: &[LambdaParam],
        body: &LambdaBody,
    ) -> String {
        let Some(Type::Fn(taken, ret)) = self.types.of(e).cloned() else {
            self.not_yet("an `async` lambda no delegate's type is known of", e.span);
            return String::new();
        };
        let names = self.lambda_names(params, taken.is_empty());
        let source_names: Vec<String> = if params.is_empty() {
            taken.iter().map(|_| IT.to_string()).collect()
        } else {
            params.iter().map(|p| p.name.text.clone()).collect()
        };
        let task_arg = match &*ret {
            Type::Own(_, args) => args.first().cloned(),
            _ => None,
        };
        let shape = Shape {
            params: source_names
                .into_iter()
                .zip(&taken)
                .map(|(n, t)| (n, self.java(t, false)))
                .collect(),
            body: match body {
                LambdaBody::Expr(x) => Body::Expr(x),
                LambdaBody::Block(stmts) => Body::Block(stmts),
            },
            gives: task_arg
                .as_ref()
                .map_or_else(|| OBJECT.to_string(), |t| self.java(t, true)),
            task: *ret != Type::Void,
            gives_value: task_arg.is_some(),
            iterator: false,
            async_iterator: false,
            enumerator: false,
        };
        // Written apart, as a lambda's body: what it computes stays inside.
        let outer = std::mem::take(&mut self.code);
        self.code.open(format!("({}) ->", names.join(", ")));
        self.machine_of(&shape);
        self.code.close("");
        let text = std::mem::replace(&mut self.code, outer).finish();
        text.trim_end().to_string()
    }

    /// The machine of `shape` made, its parameters given, started; its task
    /// returned (none for an `async void`).
    pub(crate) fn machine_of(&mut self, shape: &Shape<'_>) {
        let outer_machine = self.machine.replace(Machine {
            gives: shape.gives.clone(),
            ..Machine::default()
        });
        let outer_this = self.this_name.clone();
        if self.this_name.is_none()
            && let Some((owner, _)) = &self.owner
        {
            self.this_name = Some(Self::ident(owner));
        }
        let depth = self.renamed.len();
        let params: Vec<(String, String)> = shape
            .params
            .iter()
            .map(|(name, ty)| {
                let field = format!("{}{PARAM_FIELD}", Self::ident(name));
                self.declare_field(ty, &field);
                self.renamed.push((name.clone(), field.clone()));
                (Self::ident(name), field)
            })
            .collect();
        let step = self.machine_step(shape);
        self.renamed.truncate(depth);
        let machine = std::mem::replace(&mut self.machine, outer_machine).unwrap_or_default();
        self.this_name = outer_this;
        // Named apart: an async lambda's machine may be inside another's.
        let n = self.temp();
        let (class, made) = (format!("{MACHINE}{n}"), format!("{MADE}{n}"));
        let base = match (shape.iterator, shape.async_iterator) {
            (true, true) => ASYNC_ITERATION,
            (true, false) => ITERATION,
            _ => ASYNC,
        };
        self.code.open(format!(
            "{FINAL} {CLASS} {class} {EXTENDS} {base}<{}>",
            shape.gives
        ));
        machine.fields.iter().for_each(|f| {
            self.code.line(f);
        });
        self.code.line(step);
        self.code.close("");
        if shape.iterator && !shape.enumerator {
            // A machine for each walk, each its own copy of the arguments.
            let source = if shape.async_iterator {
                ASYNC_ITERATION_SOURCE
            } else {
                ITERATION_SOURCE
            };
            self.code.open(format!("{RETURN} {NEW} {source}<>(() ->"));
            self.code
                .line(format!("{FINAL} {VAR} {made} = {NEW} {class}();"));
            for (param, field) in &params {
                self.code.line(format!("{made}.{field} = {param};"));
            }
            self.code.line(format!("{RETURN} {made};"));
            self.code.close(");");
            return;
        }
        self.code
            .line(format!("{FINAL} {VAR} {made} = {NEW} {class}();"));
        for (param, field) in &params {
            self.code.line(format!("{made}.{field} = {param};"));
        }
        if shape.iterator {
            // An enumerator: the machine itself, walked once.
            self.code.line(format!("{RETURN} {made};"));
        } else if shape.task {
            self.code.line(format!("{made}.{RUN}();"));
            self.code.line(format!("{RETURN} {made}.{TASK}();"));
        } else {
            self.code.line(format!("{made}.{START}();"));
        }
    }

    /// The machine's `$step`: its code in states, finishing with nothing
    /// when it ends without `return`.
    fn machine_step(&mut self, shape: &Shape<'_>) -> String {
        let outer = std::mem::take(&mut self.code);
        self.code
            .open(format!("{OVERRIDE} {PROTECTED} {VOID} {STEP}()"));
        self.code.open(format!("{LOOP}: {WHILE} ({TRUE})"));
        self.code.open(format!("{SWITCH} ({STATE})"));
        self.code.line(format!("{CASE} 0:"));
        match shape.body {
            Body::Block(stmts) => self.machine_stmts(stmts, shape.gives_value),
            Body::Expr(e) if shape.gives_value => {
                if !self.await_unwritable(e) {
                    let v = self.handed_expr(e);
                    self.async_return(Some(v));
                }
            }
            Body::Expr(e) => {
                if !self.await_unwritable(e) {
                    self.expr_stmt(e);
                }
            }
        }
        self.async_return(None);
        self.code.line(format!("{DEFAULT}:"));
        self.code.line(format!("{RETURN};"));
        self.code.close("");
        self.code.close("");
        self.code.close("");
        let text = std::mem::replace(&mut self.code, outer).finish();
        text.trim_end().to_string()
    }

    /// `return v;` in the machine: its task finished with `v` (nothing's `null`).
    pub(crate) fn async_return(&mut self, v: Option<String>) {
        let v = v.unwrap_or_else(|| NULL.to_string());
        self.code
            .line(format!("{IF} ({TRUE}) {{ {FINISH}({v}); {RETURN}; }}"));
    }

    /// A new state of the machine (none outside one: `0`).
    pub(crate) fn new_state(&mut self) -> usize {
        match &mut self.machine {
            Some(m) => {
                m.states += 1;
                m.states
            }
            None => 0,
        }
    }

    /// Where state `state` starts.
    pub(crate) fn state_starts(&mut self, state: usize) {
        self.code.line(format!("{CASE} {state}:"));
    }

    /// Going on from state `state` (always taken; what follows reached by a `case`).
    pub(crate) fn jump(state: usize) -> String {
        format!("{IF} ({TRUE}) {{ {STATE} = {state}; {CONTINUE} {LOOP}; }}")
    }

    /// A field of the machine, of Java's type `ty`.
    pub(crate) fn declare_field(&mut self, ty: &str, name: &str) {
        if let Some(m) = &mut self.machine {
            m.fields.push(format!("{ty} {name};"));
        }
    }

    /// The field a name of the method is, when it's one.
    pub(crate) fn field_of(&self, name: &str) -> Option<&str> {
        self.renamed
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f.as_str())
    }
}

impl Gen<'_> {
    /// `this` where the code is: its method's class's in a machine.
    pub(crate) fn this_ref(&self) -> String {
        let this = match &self.this_name {
            Some(owner) => format!("{owner}.{THIS}"),
            None => THIS.to_string(),
        };
        // An `extend`'s `this`: its owner's handle.
        if self.extending {
            return format!("{this}.{}", yog_script_ids::jvm::extend::OWNER);
        }
        this
    }

    /// `base` where the code is: its method's class's in a machine.
    pub(crate) fn base_ref(&self) -> String {
        match &self.this_name {
            Some(owner) => format!("{owner}.{SUPER}"),
            None => SUPER.to_string(),
        }
    }
}

