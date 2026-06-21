# Licensing strategy

> Goal: keep Yog free and prevent anyone from shipping a **closed-source /
> proprietary** fork of the loader — without strangling the mod ecosystem.

## Important: copyleft does not forbid "earning"

AGPL/GPL does **not** stop others from charging money (Red Hat sells GPL
software). What copyleft stops is a **proprietary fork** — anyone who
distributes a modified loader must release their source under the same license.
So a competitor cannot take Yog, close it, and sell a closed product. That is
the protection you actually want, and it keeps Yog open source.

If the goal were instead "nobody may ever charge anything," that needs a
*non-commercial, source-available* license — which is **not** open source,
can't go on crates.io as OSS, and contradicts the "OSS + donations" plan. We are
**not** doing that.

## The split (why two licenses)

If the crate that **mods link against** (`yog-api`) were AGPL, every mod would
become a derivative work and be forced to AGPL too — killing the ecosystem (the
whole point is letting people write mods freely). This is why GCC has a runtime
exception, Java has the Classpath exception, etc.

So:

| Component | License | Why |
|-----------|---------|-----|
| `yog-runtime` (cdylib engine) | **AGPL-3.0-only** | the valuable engine; no closed forks |
| Fabric host (`dev.yog`, Java) | **AGPL-3.0-only** | part of the loader |
| `yog-api` (mods depend on this) | **MIT OR Apache-2.0** | mods may use any license |
| `yog-example-mod` | **MIT OR Apache-2.0** | copy-paste template |

A competitor can't rebuild a closed loader from the permissive API alone — the
real work lives in the AGPL runtime/host.

## Open questions to finalise

- [ ] Keep `yog-api` permissive (MIT/Apache) **or** make it
      `AGPL-3.0 WITH a linking exception` (Classpath-style)? Permissive is
      simpler and friendlier; an exception keeps it copyleft while still freeing
      mods. **Recommendation: permissive.**
- [ ] Minecraft gray area: GPL-family licenses vs linking against proprietary
      Minecraft is legally murky in modding. Confirm comfort, or consider
      `LGPL`/`MPL-2.0` for the engine if AGPL proves awkward in practice.
- [ ] Add the actual `LICENSE` files (AGPL-3.0 + MIT + Apache-2.0 texts).
