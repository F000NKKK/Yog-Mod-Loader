//! `Task`: the interpreter runs an `async` method to its end, so every task
//! is done (`Value::Done`) — made of a result, waited for at once, its
//! result had.

use yog_script_ids::members::{
    COMPLETED_TASK, CONFIGURE_AWAIT, DELAY, FROM_RESULT, GET_AWAITER, GET_RESULT, IS_COMPLETED,
    RESULT_MEMBER, WAIT, WHEN_ALL, WHEN_ANY, YIELD_MEMBER,
};
use yog_script_ids::system::TASK;

use super::Native;
use crate::value::Value;

/// `Task.name(args)`.
pub(super) fn call_static(of: &str, name: &str, args: &[Value]) -> Option<Native> {
    if of != TASK {
        return None;
    }
    let v = match (name, args) {
        (FROM_RESULT, [v]) => Value::done(v.clone()),
        (YIELD_MEMBER | DELAY, _) => Value::done(Value::Void),
        (WHEN_ALL, items) => Value::done(Value::list(each(items).iter().map(result).collect())),
        (WHEN_ANY, items) => Value::done(each(items).first().cloned()?),
        _ => return None,
    };
    Some(Ok(v))
}

/// `Task.CompletedTask`.
pub(super) fn static_member(of: &str, name: &str) -> Option<Value> {
    (of == TASK && name == COMPLETED_TASK).then(|| Value::done(Value::Void))
}

/// A task's `name`: its result, that it's done.
pub(super) fn member(task: &Value, name: &str) -> Option<Value> {
    match name {
        RESULT_MEMBER => Some(result(task)),
        IS_COMPLETED => Some(Value::Bool(true)),
        _ => None,
    }
}

/// A task's `name(args)`: waited for (done already), its awaiter its own.
pub(super) fn call(task: &Value, name: &str, args: &[Value]) -> Option<Native> {
    let v = match (name, args) {
        (WAIT, _) => Value::Void,
        (GET_RESULT, []) => result(task),
        (CONFIGURE_AWAIT | GET_AWAITER, _) => task.clone(),
        _ => return None,
    };
    Some(Ok(v))
}

/// What a task finished with (another value: itself).
fn result(task: &Value) -> Value {
    match task {
        Value::Done(v) => (**v).clone(),
        other => other.clone(),
    }
}

/// `params` items: given one by one, or as one list.
fn each(items: &[Value]) -> Vec<Value> {
    match items {
        [Value::List(all)] => all.borrow().clone(),
        many => many.to_vec(),
    }
}
