Rust bindings for Enduro/X.

# Events, request logging and DLM

Event subscriptions require a destination service. Configure `TpEvCtl::set_name1`
and `set_flags(TPEVSERVICE)`, then call
`ctx.tpsubscribe(expression, filter, &ctl, flags)` from a server. `tppost` returns
the number of servers that consumed the event; `tpunsubscribe` returns the number
removed. `TPEVPERSIST` retains a subscription
when its destination service is unavailable.

Request logging uses `AtmiCtx::tplogsetreqfile(data, filename, filesvc)`, with optional
parameters. A UBF carries the request filename; a filename service can replace it,
including on a service failure. Use `tploggetbufreqfile`/`tplogdelbufreqfile` for that
field, `tploggetreqfile` for the active request log, and `tplogclosereqfile` to close
it. `tplogsetreqfile_direct` selects a file without a buffer, and `tplogclosethread`
closes thread loggers. Use a request filename distinct from the process log when
threads are involved. All three logging macros honor the context under `ctx-send`.

The `dlm` module exports the native `EX_DLM_*` fields and `NDRX_DLM_*` constants.
Prepare a request before calling the DLM service:

```rust,no_run
use endurox_rs::{AtmiCtx, TpDlmCtl, dlm::*};

fn lock(ctx: &AtmiCtx) -> Result<(), Box<dyn std::error::Error>> {
    let mut request = ctx.tpalloc_ubf(1024)?;
    request.bchg(EX_DLM_OP, 0, NDRX_DLM_OP_DLMCMD, true)?;
    request.bchg(EX_DLM_CMD, 0, NDRX_DLM_CMD_LOCK, true)?;
    request.bchg(EX_DLM_KEY, 0, "example-key", true)?;
    request.bchg(EX_DLM_LOCK_MODE, 0, NDRX_DLM_LM_EXCLUSIVE, true)?;
    let mut ctl = TpDlmCtl::default();
    ctl.set_dlmspace("MYSPACE")?.set_wait_time(2500)?;
    ctx.tpdlmmkcall(&mut request, &mut ctl)?;
    let mut reply = ctx.tpalloc_ubf(1024)?;
    ctx.tpdlmcall(&ctl.svcname(), &mut request, &mut reply, 0)?;
    Ok(())
}
```

`tpdlmacall` submits with native retries and returns a descriptor for `tpgetrply`;
`tpdlmconnect` retries conversation establishment. These are synchronous native
operations. `tpdlmtoutget` returns total/per-attempt milliseconds and block seconds;
`tpdlmattemptsget` returns the native attempt count. The core owns retry policy and
enforces `TPNOTRAN`. DLM operations require the native `tpdlmsv` service; the Rust
integration fixture validates the call protocol against a test service.

Preparation supports flat and pointer-command layouts, validates unique ownership,
and preserves partial changes on failure. The matching core must preserve nested
command pointers when preparation fails after reallocation (`libatmi/eeapi.c`).

# UBF occurrence search

`TypedUbf::bfindocc(field, &value, regex)` searches using the exact native field
type; `regex` enables full STRING regular-expression matching. No match returns
`BNOTPRES`. `bfindlast` returns `(occurrence, UbfFieldRef)` with borrowed variable-size
data. `bgetlast` returns `(occurrence, UbfValue)` with independent complex-buffer
copies; NULL PTR/VIEW occurrences return `BNOTPRES`. `AtmiCtx::bneeded(count, bytes)`
estimates UBF allocation size from a positive field count and total value size.

# Mapping Rust structures to UBF and VIEW

`UbfSerialize` / `UbfDeserialize` map named Rust structs to native UBF fields.
These are the binding's own derive traits, not implementations of the generic
`serde::Serialize` / `serde::Deserialize` traits. No async feature is required.

```rust,no_run
use endurox_rs::{AtmiCtx, UbfDeserialize, UbfSerialize, ubf_fields as f};

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct Item {
    #[ubf(field = f::T_LONG_FLD)]
    id: i64,
    #[ubf(field = f::T_STRING_FLD)]
    name: String,
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct Message {
    // Each Item occupies one inline BFLD_UBF occurrence, starting at zero.
    #[ubf(field = f::T_UBF_FLD, nested)]
    items: Vec<Item>,
    // Each present Item gets its own owned UBF allocation behind BFLD_PTR.
    // NULL occurrences preserve None entries, including a trailing None.
    #[ubf(field = f::T_PTR_FLD, ptr)]
    linked: Vec<Option<Item>>,
    // A fixed window: occurrences 2 and 3, preserving other occurrences.
    #[ubf(field = f::T_SHORT_FLD, occ = 2)]
    flags: [i16; 2],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    let mut buffer = ctx.tpalloc_ubf(256)?;
    let message = Message {
        items: vec![Item { id: 1, name: "inline".into() }],
        linked: vec![Some(Item { id: 2, name: "pointer".into() }), None],
        flags: [3, 4],
    };
    buffer.ubf_write(&message, true)?; // Allow the UBF allocations to grow.
    assert_eq!(buffer.ubf_read::<Message>()?, message);
    Ok(())
}
```

The runnable [complex_ubf example](examples/complex_ubf.rs) also demonstrates
deep cloning and reading after the original buffer is dropped.

Use your application's generated field constants in place of the bundled test
fields. Nested structs can themselves contain scalar, inline UBF, pointer, and
VIEW fields, but an **inline** (`nested`) child must be pointer-free: a
sub-struct that owns `BFLD_PTR` targets has to be stored behind `ptr` instead,
because inline-embedding would byte-copy its pointers and share the targets.
`Box<T>` supports recursive structures, such as a
`#[ubf(field = ..., ptr)] next: Option<Box<Node>>` field. Generic structs and
borrowed fields for serialization are supported; deserialization produces owned
Rust values and leaves the native buffer readable.

| UBF field attribute | Storage and Rust mapping |
| --- | --- |
| `field = ID` | Scalar, `String`, `bool`, or `UbfCarray`; also `Option<T>`, `Vec<T>`, and `[T; N]` |
| `field = ID, nested` | `BFLD_UBF`, pointer-free child implements `UbfSerialize` / `UbfDeserialize` |
| `field = ID, ptr` | `BFLD_PTR` pointing to an owned UBF child |
| `field = ID, view` | `BFLD_VIEW`, child implements `ViewSerialize` / `ViewDeserialize` |
| `field = ID, ptr, view` | `BFLD_PTR` pointing to an owned VIEW child |
| `flatten` | Delegate a child struct to the same parent buffer at occurrence zero |
| `group` | Flat: a `#[ubf(group)]` child's fields feed from grouped occurrences of the parent's own fields; a `Vec`/array walks the occurrence per element |
| `field = ID, default` | Use Rust `Default` only when the starting outer occurrence is absent |
| `skip` | Ignore when writing; use Rust `Default` when reading |

`occ = N` (also spelled `occurrence`) selects the starting occurrence; the default
is zero. `size = N` selects the initial child allocation for `nested` and UBF
`ptr` mappings. Storage attributes also accept `Option<T>`, `Vec<T>`, and `[T; N]`.
For renamed dependencies, set `#[ubf(crate = "your_alias")]` or
`#[view(crate = "your_alias")]` on the struct.

Occurrences have explicit ownership rules:

- A scalar or struct writes one occurrence. An array writes exactly its fixed
  window and preserves occurrences outside that window.
- A vector reads all occurrences from its starting offset onward. Writing a
  shorter vector removes the stale tail. Avoid overlapping that suffix with
  another mapped field.
- A top-level optional field reads absence as `None`; writing `None` removes
  its occurrence and everything after it. In repeated pointer and embedded VIEW
  mappings, `Option<T>` instead uses a native NULL placeholder, preserving gaps.
- Repeated inline UBF and scalar fields have no unambiguous NULL marker.
  Wrap optional elements or inner collections in a mapped struct instead of
  using `Vec<Option<T>>` or `Vec<Vec<T>>` there. Use `UbfCarray(Vec<u8>)` for one
  binary field; `Vec<u8>` means repeated numeric occurrences.

Integer mappings check signedness and range, including the native field width;
unsigned values must fit the signed XATMI long representation. Booleans use
exactly 0 and 1. Invalid child contents return an error even with `default`.
Errors include the mapped struct/field path. Writes can partially update a
buffer on error; build a fresh buffer when an all-or-nothing update is needed.

### Flat groups (`#[ubf(group)]`)

`nested`/`ptr`/`view` give each sub-struct its own sub-buffer. A flat **group**
instead lays a sub-struct out directly in the parent buffer: each of its fields
occupies the same occurrence column, so a `Vec` of them maps to the classic
"struct of arrays" UBF layout with no `BFLD_UBF`/`BFLD_PTR` at all. Opt a struct
in with a container `#[ubf(group)]`, then map it flat with a field-level
`#[ubf(group)]`:

```rust
#[derive(UbfSerialize, UbfDeserialize)]
#[ubf(group)]                                   // this struct can lay out flat
struct Account {
    #[ubf(field = f::T_ACCT_NO_FLD)]  number: String,   // anchor (first field)
    #[ubf(field = f::T_ACCT_BAL_FLD)] balance_cents: i64,
}

#[derive(UbfSerialize, UbfDeserialize)]
struct Customer {
    #[ubf(field = f::T_CUST_ID_FLD)] id: i64,
    #[ubf(group)] accounts: Vec<Account>,       // account i ⇄ occurrence i
}
```

`account[i].number` is `T_ACCT_NO_FLD[i]`, `account[i].balance_cents` is
`T_ACCT_BAL_FLD[i]`, and so on, all in one flat buffer. On read the element count
comes from the group's **anchor** — its first mapped field — so that field must
be present once per element; writing a shorter vector truncates every group
field's tail. A group element may still redirect one of its *own* fields into a
per-element sub-buffer with `nested`/`ptr`/`view` (each such field takes one
occurrence per element). Every member must occupy exactly one occurrence;
vectors, arrays and optional scalar/inline-UBF members are rejected in a group.
Wrap those values in a nested struct. Optional pointer/VIEW members use NULL
placeholders, including when they are the anchor. A member's `occ` offset is
added to the group's base for writing, reading and tail cleanup; the anchor's
offset is excluded from its row count. Negative or overflowing offsets return
an error before the group is written.

A group may not contain another `group` or `flatten` field,
because a flat buffer has only one occurrence axis. Because a group is plain
occurrences (no pointers), it marshals across `tpcall` intact.

## Compiled VIEW layouts

Derive `ViewSerialize` and `ViewDeserialize` for a struct matching a compiled
Enduro/X VIEW. Field names default to Rust member names; override them with
`#[view(field = "native_name", occ = N)]`. `flatten` and `skip` are also available.
The layout must be compiled and loaded through the usual `VIEWDIR` / `VIEWFILES`
configuration; the derive does not generate a native layout.

```rust
use endurox_rs::{UbfDeserialize, UbfSerialize, ViewDeserialize, ViewSerialize,
                 ubf_fields as f};

#[derive(ViewSerialize, ViewDeserialize)]
#[view(name = "POSITION")]
struct Position {
    id: i64,
    samples: Vec<i64>,
    note: Option<String>,
}

#[derive(UbfSerialize, UbfDeserialize)]
struct Positions {
    #[ubf(field = f::T_VIEW_FLD, view)]
    inline: Vec<Position>,
    #[ubf(field = f::T_PTR_FLD, ptr, view)]
    linked: Option<Position>,
}
```

For example, the corresponding VIEW source can be:

```text
VIEW POSITION
# type  cname    fbname count flags size null
long    id       -      1     -     -    -
long    samples  -      8     C     -    -
string  note     -      1     -     64   -
END
```

Use `TypedView::view_write` / `view_read` for standalone VIEW buffers. Variable
length vectors require a native `C` count indicator, unless they fill the entire
declared array. Binary fields use `UbfCarray`; add an `L` length indicator to
preserve variable byte lengths. Optional VIEW members use the layout's NULL
sentinel: `None` requires one, and `Some(value)` equal to that sentinel is rejected
because it could not round-trip distinctly. Fixed arrays can contain optional
members and use the declared occurrence capacity.

## Buffer ownership

The parent owns pointer targets created by these mappings. Replacing a pointer,
removing optional fields, or shortening a collection frees the removed owned
targets, including pointers inside embedded UBFs. Inline-embedding (`nested`) a
UBF that itself owns `BFLD_PTR` targets is rejected, because the inline copy
would share those targets with the consumed original; store such sub-structures
behind `ptr`. Reads validate native buffer types and do not extract targets.
Nested structure reads reject cycles and traversal beyond 128 child levels.

`TypedUbf::deep_clone()` copies inline fields and recursively allocates independent
pointer targets. Shared targets are copied separately; cyclic or excessively
deep graphs return an error. Destructive owned edits reject shared/cyclic native
pointer graphs so that a surviving occurrence cannot become dangling. Clone an
acyclic shared graph before editing. Native-style shallow copying still rejects
pointer fields. `UbfAdhoc` can serialize an unmapped embedded UBF (including one
that owns pointers) by deep copy; use `ubf_read_adhoc` for scoped access when
reading it.

# Scripting plugins

The native `tpscr*` API is available through `AtmiCtx::tpscrinit` and the returned
`TpScrVm`. Enduro/X selects the language backend through `NDRX_PLUGINS`; Rust
does not embed or link directly to Python. See the runnable
[scripting example](examples/scripting.rs) for Python calling a Rust closure.

```rust,no_run
use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrSlot, NDRX_TPSCR_FLAT};
# fn example() -> Result<(), Box<dyn std::error::Error>> {
let ctx = AtmiCtx::new()?;
ctx.tpinit()?;
let mut vm = ctx.tpscrinit(None, 0)?;
vm.tpscrregcb("host", |call, args| {
    args.set(TpScrSlot::Output,
             Some(call.context().tpalloc_carray(b"hello from Rust")?))
}, 0)?;
vm.tpscrloadstr("example",
    "def main(name, param, incoming, flags):\n    return host(param, incoming, flags)\n",
    NDRX_TPSCR_FLAT)?;
let mut buffers = TpScrBuffers::new(&ctx);
vm.tpscrexec("example", &mut buffers, 0)?;
let output = buffers.take(TpScrSlot::Output)?.unwrap();
assert_eq!(output.as_bytes(), b"hello from Rust");
# Ok(())
# }
```

- `tpscrcomp` returns `TpScrBytecode`, which exposes `as_bytes()` and frees
  compiler storage with native `tpscrfree` on drop. `tpscrfree()` releases it
  explicitly; `tpscrload` also accepts serialized byte slices. Compiler output
  uses the plugin allocator, not `tpfree`.
- `tpscrloadstr`, `tpscrload`, `tpscrunload`, `tpscrexec`, `tpscrregcb`,
  `tpscrunregcb`, `tpscrerrno`, `tpscrerror`, and `tpscrseterror` are VM methods.
  `tpscruninit` consumes the VM; Drop also shuts it down. Load flags
  `NDRX_TPSCR_PACKAGE`, `NDRX_TPSCR_REPLACE`, and `NDRX_TPSCR_FLAT` are exported.
- `TpScrBuffers` owns the parameter, input and output slots. `set` replaces one
  slot, `alias` shares an allocation between slots, and `take` transfers its
  ownership while clearing all aliases. `edit` and `edit_ubf` allow mutation and
  growth while updating aliases, including during unwinding. A returned input
  buffer is freed once, even when it also occupies the output slot.
  Byte views expose CARRAY payloads and terminated STRING/JSON contents;
  structured UBF/VIEW buffers keep Rust's tracked byte length at zero so their
  padding and unused capacity are not exposed. Use typed access for those buffers.
- Execution updates buffer pointers and lengths on success and failure. Inspect
  the same `TpScrBuffers` after an error to retrieve a failure output or a
  relocated input. `TpScrError` retains both native and engine error codes.
- Registered closures may capture owned Rust state. Their callback context
  exposes the current `AtmiCtx`, callback name/flags, script error APIs, and nested
  `tpscrexec` when the backend supports it. Rust panics are caught and reported as
  scripting failures. Native TLS is restored after callbacks under `ctx-send`.
- VMs and callbacks are local to their creating thread; callbacks must be
  synchronous. A VM borrows its `AtmiCtx`. Default configuration is `None`;
  `TpScrCfg::from_raw` is an unsafe escape hatch for backend-specific
  configuration because the portable core leaves that structure opaque.

The scripting integration fixture checks `ENDUROX_TPSCRIPT_PLUGIN`, installed
Python packages discoverable by interpreters on `PATH` (including user site
packages), then the sibling Python binding's build/source directories. `PYTHON`
selects the first interpreter to probe. It prints the selected shared object;
check this path when an old installation behaves differently from current
plugin source. Missing-plugin errors are tested independently. If no Python
plugin is found, the language integration portion reports a skip.

# Runtime modes

The default build has no async-runtime dependency. Use `AtmiCtx::tpcall` for
the normal blocking XATMI call path, or `AtmiCtx::tpacall` plus
`AtmiCtx::tpgetrply` when managing Enduro/X call descriptors directly.

Async support uses a runtime-neutral call state machine with an explicitly
selected reply-fd driver. Async features also enable `ctx-send`, ensuring each
adapter owns a distinct Enduro/X Object API context. Tokio-native integration
is optional:

```toml
[dependencies]
endurox-rs = { version = "0.1", features = ["tokio"] }
tokio = { version = "1", features = ["macros", "net", "rt", "time"] }
```

For an executor-independent driver backed by the `async-io` reactor instead:

```toml
endurox-rs = { version = "0.1", features = ["async-io"] }
```

The `async-io` adapter's futures can be polled by Tokio, smol, async-std, or
another standard Rust executor. Enabling both `tokio` and `async-io` is also
supported; the adapter type determines which reactor a context uses.

Async waiting requires an Enduro/X build whose `ndrx_config.h` selects
`EX_USE_EPOLL` or `EX_USE_KQUEUE`. On other queue backends adapter construction
returns `TPEINVAL`; use the blocking API from the runtime's blocking-task
facility and create the complete `AtmiCtx` inside that task.

`AtmiCtx` is deliberately `!Sync`, so its async call futures are `!Send`. Await
them directly on a current-thread runtime or a `tokio::task::LocalSet`, rather
than passing them to `tokio::spawn`:

```rust,no_run
use endurox_rs::AtmiCtx;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    let ctx = ctx.into_tokio()?;

    let request = ctx.tpalloc_carray(b"request")?;
    let mut response = ctx.tpalloc_carray(&[])?;
    ctx.tpcall("MY_SERVICE", &request, &mut response, 0).await?;

    drop(response);
    drop(request);
    ctx.tpterm()?;
    Ok(())
}
```

Async `tpcall`, `tpacall`, and `tpgetrply` take the same arguments as their
blocking counterparts and must be awaited. There is no per-call timeout
argument: timeouts come from `NDRX_TOUT`, `tptoutset` and `tpsblktime`. The
adapter captures the effective deadline before the first send attempt, and
retains it through queue waiting and reply collection. For a one-off override,
use the same idiom
you would use before a blocking call:

```rust,no_run
# use endurox_rs::{AtmiCtx, TPBLK_NEXT};
# async fn f(ctx: &endurox_rs::TokioAtmiCtx, req: &endurox_rs::TypedBuffer<'_>,
#            rsp: &mut endurox_rs::TypedBuffer<'_>) -> Result<(), endurox_rs::AtmiError> {
ctx.tpsblktime(3, TPBLK_NEXT)?;
ctx.tpcall("MY_SERVICE", req, rsp, 0).await?;
# Ok(())
# }
```

Both async drivers handle full destination queues without blocking the executor
in the native queue send. Native send attempts use `TPNOBLOCK`; when the queue is
full, ordinary calls yield through timer waits of 1, 2, 4, 8, then at most 16 ms
before another attempt. These are timer-based retries, not destination-writable
notifications. Other tasks can run during each wait. Only send-side `TPEBLOCK`
is retried; successful sends and other errors are never resubmitted. The adapter
also drains and routes pending replies while waiting for send space, preventing
a full client reply queue from blocking the service that must consume requests.

Explicit `TPNOBLOCK` requests make just one send attempt and return `TPEBLOCK`
immediately if the destination queue is full. For `tpcall`, a successful send
still awaits its reply; for `tpgetrply`, `TPNOBLOCK` continues to mean “return
immediately if no reply is available.” `TPNOTIME` disables the adapter deadline;
dropping a future still cancels its waiting.

Submit and collect separately with:

```rust,no_run
# use endurox_rs::{TokioAtmiCtx, TypedBuffer, AtmiError};
# async fn submit(ctx: &TokioAtmiCtx, request: &TypedBuffer<'_>,
#                 response: &mut TypedBuffer<'_>) -> Result<(), AtmiError> {
let mut cd = ctx.tpacall("MY_SERVICE", request, 0).await?;
ctx.tpgetrply(&mut cd, response, 0).await?;
# Ok(())
# }
```

`AsyncAtmiCtx::tpacall` is now awaitable; update older adapter call sites to add
`.await`, or use `try_tpacall` for one immediate send attempt. `tpacall_async` is
an alias for the awaitable method. `AtmiCtx::tpacall` remains the native synchronous
submission API. With `TPNOREPLY`, async submission returns zero once sent.

`AsyncAtmiCtx::tpsprio(priority, flags)` configures the next async submission,
including `try_tpacall`. Use flags 0 for a relative priority or `TPABSOLUTE` for
an absolute priority from 1 to 100. The setting is consumed when the submission
is first polled. Its resolved absolute priority is kept across retries, and a
priority configured by another task remains available for that task's next
submission. These adapter settings apply to async submissions; configure native
operations invoked through `context()` with `context().tpsprio(...)`. Converting
back with `into_inner()` transfers an unused adapter priority to the native
context.

Concurrent async calls on one context share a single reactor registration.
Replies are demultiplexed by call descriptor using `tpgetrply(TPGETANY)`, so a
reply wakes exactly the future waiting for it, and no reply is ever diverted
into Enduro/X's in-memory queue where the reply fd could not signal it again.

Dropping a send future while it waits for queue space stops all further attempts.
Dropping `AsyncAtmiCtx::tpcall` after submission cancels its call descriptor; dropping
`AsyncAtmiCtx::tpgetrply` leaves its caller-owned descriptor pending so it can
be awaited again or cancelled explicitly. Multiple adapters may share one
executor thread; each owns a separate Enduro/X context and reply queue.

The generic form is `AsyncAtmiCtx<D>`, where `D: AsyncReplyDriver`. The provided
drivers are `TokioReplyDriver` and `AsyncIoReplyDriver`. Because the adapter
owns `AtmiCtx`, a context cannot accidentally be registered with two reactors.

# Server dispatch threads

`AtmiCtx::tp_run` enables Enduro/X's multithread-capable integration mode. When
`maxdispatchthreads` is greater than one, `mindispatchthreads` worker threads
are created and service callbacks may execute concurrently on any of them.
Each callback receives a worker-local `AtmiCtx`; with `ctx-send` it temporarily
uses that worker's OAPI context and restores the worker TLS before returning.
With `maxdispatchthreads=1`, callbacks stay on the `tp_run` main thread and use
the owning server context; worker init/done hooks are intentionally not called.
With `mindispatchthreads=1` and `maxdispatchthreads>1`, one worker is created and
its thread init/done hooks are called normally.

Rust integration also uses `ATMI_SRVLIB_NOLONGJUMP`, so `tpreturn` and
`tpforward` return through Rust normally instead of performing a C `longjmp`
across Rust stack frames. Service handlers must protect any shared application
state because the same handler function can run concurrently.

# Testing

Sync mode tests:

```
$ cargo test
```

Context-migration tests:

```
$ cargo test --features ctx-send
```

Tokio API tests:

```
$ cargo test --features tokio
```

Executor-independent async API tests:

```
$ cargo test --features async-io
```
