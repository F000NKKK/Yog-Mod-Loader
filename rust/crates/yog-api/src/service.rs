//! Services through the bus: a mod answers calls to an address
//! ([`Registry::provide`]), and calls whoever answers one ([`call`]).

use std::os::raw::c_void;
use yog_sync::Arc;

use yog_abi::{YogApi, YogBytes, YogReply, YogStatus, YogStr, status};
use yog_data::Schema;
use yog_event::{EventPhase, Message, Service, keyed};

use crate::bus::{EventCx, Subscription};
use crate::registry::{Registry, global_api, guard};

/// The bus's handler for provider `ud`: the request read as `S`, the
/// mod's answer written as the reply.
unsafe extern "C" fn on_call<S: Service, F>(ud: *mut c_void, payload: YogBytes, reply: *const YogReply) -> YogStatus
where
    F: Fn(&S, &EventCx) -> S::Reply + Send + Sync,
{
    let s = unsafe { &*(ud as *const Subscription<S, F>) };
    let Some(request) = S::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    let Some(answer) = guard(None, || Some((s.handler())(&request, &s.cx()))) else {
        return status::FAILED;
    };
    unsafe { YogReply::send(reply, &answer.to_bytes()) };
    status::OK
}

impl Registry {
    /// Answers every call to `S` — one provider per address: `false` if
    /// someone answers it already.
    pub fn provide<S: Service + 'static>(&mut self, handler: impl Fn(&S, &EventCx) -> S::Reply + Send + Sync + 'static) -> bool {
        self.providing::<S, _>(S::ADDRESS, Arc::new(handler))
    }

    /// [`Registry::provide`] for a keyed service's `key` only (`S::KEY`):
    /// the inserts into one block.
    pub fn provide_key<S: Service + 'static>(&mut self, key: &str, handler: impl Fn(&S, &EventCx) -> S::Reply + Send + Sync + 'static) -> bool {
        self.providing::<S, _>(&keyed(S::ADDRESS, key), Arc::new(handler))
    }

    /// Provides `address` with a raw bus handler (a `#[yog_export]`'s).
    pub(crate) fn provide_raw(&mut self, address: &str, handler: yog_abi::YogHandlerFn) -> bool {
        let bus = unsafe { &*self.api().bus };
        let st = unsafe {
            let at = (bus.resolve)(bus.ctx, YogStr::from_str(address));
            (bus.provide)(bus.ctx, at, std::ptr::null_mut(), handler)
        };
        if st != status::OK {
            crate::error!("{address}: someone provides it already");
        }
        st == status::OK
    }

    /// Describes service `address` on the bus (`bus/declare`): what
    /// `bus/describe` answers, what `bus/find` finds by `interfaces`.
    pub(crate) fn declare(&self, address: &str, doc: &str, shape: &yog_data::Shape, reply: &yog_data::Shape, interfaces: Vec<String>) {
        use yog_event::bus::{AddressKind, Descriptor};
        let d = Descriptor { address: address.to_owned(), kind: AddressKind::Service, doc: doc.to_owned(), shape: shape.describe(), reply: Some(reply.describe()), interfaces };
        self.call(&d);
    }

    fn providing<S: Service + 'static, F>(&mut self, address: &str, handler: Arc<F>) -> bool
    where
        F: Fn(&S, &EventCx) -> S::Reply + Send + Sync + 'static,
    {
        let api = self.api();
        let bus = unsafe { &*api.bus };
        let sub = Box::into_raw(Box::new(Subscription::<S, F>::new(handler, EventPhase::Post)));
        let st = unsafe {
            let at = (bus.resolve)(bus.ctx, YogStr::from_str(address));
            (bus.provide)(bus.ctx, at, sub.cast::<c_void>(), on_call::<S, F>)
        };
        if st != status::OK {
            crate::error!("{address}: someone provides it already");
            return false;
        }
        let doc = match S::shape() {
            yog_data::Shape::Struct(s) => s.doc,
            _ => "",
        };
        self.declare(S::ADDRESS, doc, &S::shape(), &S::Reply::shape(), Vec::new());
        true
    }
}

/// Calls `request`'s provider: its answer, `None` when no one answers (a
/// service not on this host, or this mod not registered yet).
pub fn call<S: Service>(request: &S) -> Option<S::Reply> {
    let api = global_api();
    if api.is_null() {
        return None;
    }
    call_on(unsafe { &*api }, request)
}

impl Registry {
    /// [`call`] while registering.
    pub fn call<S: Service>(&self, request: &S) -> Option<S::Reply> {
        call_on(self.api(), request)
    }
}

fn call_on<S: Service>(api: &YogApi, request: &S) -> Option<S::Reply> {
    let reply = request.with_bytes(|payload| call_bytes(api, request.address(), payload))?;
    S::Reply::from_bytes(&reply)
}

/// Calls `address`'s provider with `payload`: its reply's bytes, `None`
/// when no one answers (or this mod isn't registered yet).
pub(crate) fn call_at(address: yog_event::Address, payload: &[u8]) -> Option<Vec<u8>> {
    let api = global_api();
    if api.is_null() {
        return None;
    }
    call_bytes(unsafe { &*api }, address, payload)
}

/// The runtime's atom table (`yog_data::atom`), for this mod's copy of
/// `yog-data`: the same numbers as every mod's and the host's.
pub(crate) const ATOMS: yog_data::atom::Table = yog_data::atom::Table { intern: atom, name: atom_name };

fn atom(name: &str) -> u32 {
    let bus = unsafe { &*(*global_api()).bus };
    unsafe { (bus.atom)(bus.ctx, YogStr::from_str(name)) }
}

fn atom_name(n: u32) -> Option<&'static str> {
    let bus = unsafe { &*(*global_api()).bus };
    let s = unsafe { (bus.atom_name)(bus.ctx, n) };
    // The runtime keeps its atoms' names for good.
    (!s.ptr.is_null()).then(|| unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(s.ptr, s.len as usize)) })
}

fn call_bytes(api: &YogApi, address: yog_event::Address, payload: &[u8]) -> Option<Vec<u8>> {
    let bus = unsafe { &*api.bus };
    let mut out = Vec::new();
    let st = unsafe { (bus.call)(bus.ctx, address, YogBytes::new(payload), &YogReply::to_vec(&mut out)) };
    (st == status::OK).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_event::block::Insert;

    /// Runs `on_call` for `handler` on `request`: its status and reply.
    fn answer<F: Fn(&Insert, &EventCx) -> bool + Send + Sync>(handler: F, request: &[u8]) -> (YogStatus, Vec<u8>) {
        let sub = Subscription::<Insert, F>::new(Arc::new(handler), EventPhase::Post);
        let mut out = Vec::new();
        let ud = (&sub as *const Subscription<Insert, F>).cast_mut().cast::<c_void>();
        let st = unsafe { on_call::<Insert, F>(ud, YogBytes::new(request), &YogReply::to_vec(&mut out)) };
        (st, out)
    }

    fn insert(item: &str) -> Insert {
        Insert { block_id: "mymod:crate".into(), pos: yog_core::BlockPos { x: 0, y: 64, z: 0 }, dimension: "minecraft:overworld".into(), item_id: item.into(), count: 1, nbt: String::new(), simulate: true }
    }

    #[test]
    fn a_provider_answers_with_its_reply() {
        let takes_stone = |i: &Insert, _: &EventCx| i.item_id == "minecraft:stone";
        let (st, out) = answer(takes_stone, &insert("minecraft:stone").to_bytes());
        assert_eq!((st, bool::from_bytes(&out)), (status::OK, Some(true)));
        let (_, out) = answer(takes_stone, &insert("minecraft:dirt").to_bytes());
        assert_eq!(bool::from_bytes(&out), Some(false));
    }

    #[test]
    fn a_bad_request_or_a_panic_isnt_an_answer() {
        let ok = |_: &Insert, _: &EventCx| true;
        assert_eq!(answer(ok, b"junk").0, status::BAD_PAYLOAD);
        let panics = |_: &Insert, _: &EventCx| -> bool { panic!("mod bug") };
        assert_eq!(answer(panics, &insert("minecraft:stone").to_bytes()).0, status::FAILED);
    }
}
