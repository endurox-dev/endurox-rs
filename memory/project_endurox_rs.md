---
name: endurox-rs project design
description: Rust binding for Enduro/X ATMI middleware — ownership model, API design, key patterns
type: project
---

Rust binding for Enduro/X XATMI middleware (similar to Tuxedo). Uses `bindgen` to wrap the C API.

**Key ownership rules:**
- `TpSvcInfo` owns the service data buffer via `Option<TypedBuffer<'ctx>>`. Service handlers call `svc.take_data_ubf()` to get ownership, then pass it to `ctx.tpreturn_ubf(...)` which consumes it (calls `into_raw()` preventing double-free).
- `tpreturn_buffer` and `tpreturn_ubf` take by value — XATMI owns the pointer after tpreturn.
- `TypedBuffer::into_raw()` uses ManuallyDrop to prevent Drop running tpfree.

**ctx-send feature:**
- Without: thread-local context (`!Send & !Sync`), calls `raw::tpXxx()` functions
- With: explicit context handle (`Send & !Sync`), calls `raw::OtpXxx(ctx_ptr, ...)` functions
- `AtmiCtx::c_ctx_ptr()` is pub(crate) and only compiled for `ctx-send`

**UBF buffer access pattern (server handler):**
```rust
fn my_svc(ctx: &AtmiCtx, svc: &mut TpSvcInfo) {
    let mut ubf = svc.take_data_ubf().unwrap();
    let val = ubf.bget_string(fld, 0).unwrap();
    ubf.bchg(fld2, 0, UbfValue::String(rsp), true).unwrap();
    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}
```

**Client pattern:**
```rust
ctx.tpinit()?;
let mut buf = ctx.tpalloc_ubf(1024)?;
buf.bchg(fld, 0, UbfValue::String("val".into()), true)?;
ctx.tpcall("SVC", &mut buf, 0)?;
let rsp = buf.bget_string(rsp_fld, 0)?;
ctx.tpterm()?;
```

**Why:** Matches endurox-go's TpRun+callback model. Service handler MUST consume buffer via tpreturn_ubf exactly once; panic handler recovers by returning remaining buffer or fresh allocation.
