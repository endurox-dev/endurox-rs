# Working in endurox-rs

Rust bindings for Enduro/X XATMI, UBF and compiled VIEW buffers. This repository
contains the runtime library, a separate proc-macro crate, examples and tests
that exercise the native middleware. Read the current implementation when
changing a contract; the details below describe the present design.

## Maintainer preferences and API conventions

- Keep responses short, professional and direct. Lead with findings or decisions
  and cite the relevant file/line when evidence matters.
- Prefer one public Rust method per logical C operation. FILE and callback I/O
  variants usually belong behind one method returning `String` or `Vec<u8>`.
  Avoid redundant public aliases or suffixes such as `_capture` and `_str` when
  the bare operation name is sufficient. Remove unused helpers after consolidation.
- When a separate low-level helper is justified, use the `_value` suffix, as in
  `cbchg_value` or `tpreturn_value`. Do not introduce a pass-through helper merely
  to follow that naming pattern; keep single-use straightforward logic inline.
- The maintainer permits breaking API changes during this development phase.
  Do not preserve obsolete signatures solely for compatibility. The release
  convention is a monotonically increasing patch number unless the user gives
  a different policy. Read current manifests rather than copying old versions
  from notes, and do not overwrite an in-progress release bump.
- The maintainer also develops Enduro/X itself. A native API addition can be
  the correct fix when a safe or efficient binding needs information unavailable
  through the current C API. The Rust and C trees are separate repositories;
  preserve their independent working changes and commits.

## Project map

| Area | Files |
| --- | --- |
| Native build discovery and generated bindings | `build.rs`, `include/wrapper.h` |
| Public exports, flags and errors | `src/lib.rs`, `src/flags.rs`, `src/errors.rs` |
| Context allocation and lifetime | `src/atmictx.rs` |
| XATMI calls, replies, queues and timeouts | `src/atmictx_xatmi.rs` |
| Async drivers, send backpressure and reply routing | `src/async_atmi.rs` |
| Scripting VM, bytecode and host callback trampolines | `src/tpscript.rs`, `src/script_buffers.rs` |
| Server dispatch, callbacks and worker contexts | `src/atmictx_srv.rs`, `src/tpsvcinfo.rs` |
| Native buffer ownership and typed access | `src/typed_buf.rs`, `src/typed_ubf.rs`, `src/typed_view.rs` |
| UBF operations and expression callbacks | `src/atmictx_ubf.rs` |
| Complex-buffer ownership, deletion and deep copying | `src/ubf_complex.rs` |
| Structure mapping runtime | `src/ubf_serde.rs`, `src/view_serde.rs` |
| Derive macros | `endurox-rs-derive/src/lib.rs` |
| Mapping documentation and example | `README.md`, `examples/complex_ubf.rs` |
| Scripting usage example | `examples/scripting.rs` |

The ownership hierarchy is `AtmiCtx` -> `TypedBuffer<'ctx>` -> typed UBF/VIEW
wrappers. Errors retain their native subsystem: `AtmiError`/`AtmiResult` for
XATMI, `UbfError`/`UbfResult` for UBF/VIEW operations, and `NstdError`/`NstdResult`
for NSTD utilities. Error codes are `u32` constants corresponding to the native
subsystem; preserve both code and diagnostic message.

## Native build environment

- The build requires installed Enduro/X headers/libraries, `pkg-config`, and
  libclang for bindgen. `build.rs` discovers `atmisrvinteg.pc`. Check
  `pkg-config --variable=includedir atmisrvinteg` and `pkg-config --libs
  atmisrvinteg` when diagnosing which installation is selected.
- Relevant overrides include `PKG_CONFIG_PATH`, `CFLAGS`, `LDFLAGS`, and
  `LIBCLANG_PATH`. Avoid hard-coding a developer's installation paths.
  For example, `CFLAGS="-I/path/to/endurox/include"` and
  `LDFLAGS="-L/path/to/endurox/lib"` supplement native discovery. The build uses
  `atmisrvinteg` package metadata; it does not have a separate `libtux` prerequisite.
- Bindings are generated into Cargo's `OUT_DIR` and included by private module
  `raw`. Change the wrapper or safe Rust implementation, not generated files.
- Test field constants are generated from `tests/ubftab/test.fd` using
  `mkfldhdr -m4`. Lookup order is `ENDUROX_MKFLDHDR`, the sibling executable
  `../endurox/mkfldhdr/mkfldhdr`, then `PATH`.
- UBF tests compile `tests/views/test.v`. Their compiler lookup uses
  `ENDUROX_VIEWC`, `../endurox/dist/bin/viewc`, then `PATH`. Runtime layouts are
  loaded through `VIEWDIR`/`VIEWFILES`; field tables use `FLDTBLDIR`/`FIELDTBLS`.
- Launchers may source `~/ndrx_home`. Inspect effective configuration rather
  than assuming the invoking shell's settings survive the launcher.
- If the sibling `../endurox` checkout exists, its `libatmi`, `libubf`,
  `libatmisrv`, headers and `doc/api` sources are useful for checking native
  contracts. A rebuilt checkout does not automatically replace the installed
  libraries or executables selected by `pkg-config` and `PATH`.
- Compare behavior with the public C entry point first (`libatmi/atmi.c` for
  XATMI, `libubf/ubf.c` for UBF), then follow its `ndrx_*` helper. Shared helpers
  can accept arguments that the public API rejects. For example, public
  `tpcall` rejects `TPNOREPLY` even though `ndrx_tpcall` supports it for other APIs.
- Prefer native helpers such as `Bhasptr` for pointer detection instead of
  rebuilding internal UBF bit layouts or scanning every field from Rust.
  Internal constants and cached field offsets belong to the native library.
- A public native UBF addition normally needs public/internal declarations,
  implementation and validated wrapper, Object API generation through
  `scripts/gen_oapi.sh`, registered `ubftest` coverage and an API manpage registered
  in the documentation CMake list. Review generated changes carefully, then build
  and install the matching native library before rebuilding the binding. Follow
  the sibling repository's own `AGENTS.md` when editing it.
- Shell commands may run under zsh, which does not split a string such as
  `"--features ctx-send"` into arguments. Use explicit commands, shell arrays,
  or functions forwarding `"$@"`; inspect command output before treating a
  nonzero exit from a shell loop as a compiler failure.

## Contexts and server callbacks

- Default `AtmiCtx` is neither `Send` nor `Sync`. Feature `ctx-send` makes it
  `Send`, still not `Sync`, using a distinct native Object API context.
- Keep the default C API and `ctx-send` Object API paths equivalent. Ordinary
  native operations under `ctx-send` use the appropriate `O...` entry points.
  Context attach/detach operations are special: do not blindly substitute
  `Otpgetctxt` for raw context management, which can detach twice.
- Buffers borrow their context. Preserve that lifetime relationship and the
  read-only lifetime of borrowed UBF/pointer views.
- Service callbacks receive worker-local context views. Those views restore
  worker TLS when dropped; they must not terminate or free the worker context.
- With multiple dispatch threads, callbacks can run concurrently. With one
  dispatch thread, callbacks use the server's main context and worker hooks
  are not called. Preserve both paths when changing server integration.
- Server integration uses `ATMI_SRVLIB_NOLONGJUMP`; native return/forward paths
  must return through Rust normally rather than jump across Rust stack frames.
- `tp_run` takes `ServerHooks`, with a main init hook of type
  `fn(&AtmiCtx, &[String]) -> AtmiResult<()>`. Register services from that hook:
  `ctx.tp_run(ServerHooks::new(server_init).done(server_done))?`.
  The former two-closure `tp_run(init, done)` example is obsolete.
- `SERVER_RUNTIME` stores dispatch and lifecycle state behind a mutex. Rust
  worker init runs after native `tpsvrthrinit`; Rust worker done runs before native
  `tpsvrthrdone` and termination. Preserve the first initialization failure for
  `tp_run` to return; convert callback panics to native failures.
- Poller-fd, periodic and before-poll callbacks run on the main poll-loop thread
  with its context. Adding/deleting these extensions from dispatch workers is
  rejected with `TPEPROTO` because the native extension list is not synchronized.

## Buffer ownership rules

- `TypedBuffer` frees owned allocations with native `tpfree`. Never create two
  Rust owners for one allocation or turn a borrowed pointer into an owner.
- `TpSvcInfo` owns the incoming request through `Option<TypedBuffer>`. Taking
  it with `take_data`/`take_data_ubf` moves ownership to the handler. Return and
  forward methods consume buffers to transfer ownership to Enduro/X exactly
  once. Panic recovery uses the still-owned request or a fresh failure buffer
  if the handler has already consumed the request.
- Allocation capacity and logical payload length are different. In particular,
  a pointer field carries an address, not a CARRAY payload length. Preserve
  native-reported output lengths; do not substitute `tptypes().size`.
- Raw byte access and length mutation are restricted to CARRAY. UBF/VIEW have
  native layouts, while STRING/JSON have terminator requirements. Keep these
  type checks and allocation bounds intact.
- Receive/call APIs can replace output pointers even when returning an error.
  Preserve pointer, length and descriptor updates before returning that error.
- `BFLD_PTR` stores a pointer value: native writes receive the address of a
  pointer variable. Successful writes transfer target ownership to the parent;
  failures must leave the new target owned by its Rust wrapper.
- `bget_ptr` and `bget_ptr_ubf` return read-only borrowed views. `bextract_ptr`
  removes the field and returns standalone ownership, allowing mutation and
  reallocation without leaving a stale address in the parent.
- Replacing/deleting owned pointer occurrences must reclaim old targets,
  including pointers inside inline UBFs. `bdel_owned` provides this behavior;
  native-style `AtmiCtx::bdel` only removes the field reference.
- Shallow copy operations reject pointer-containing buffers. Public inline
  embedding through `bchg`/`nested` also requires a pointer-free child. Use a
  `ptr` mapping for a child that itself owns pointer targets.
- Internal `put_owned_ubf` is a separate transfer path used for fresh deep
  copies. After copying inline bytes, it clears the consumed shell with `Binit`
  before dropping it. Losing that step frees targets now owned by the parent.
- `deep_clone` recursively allocates independent targets. Repeated references
  become separate copies; cycles and excessive nesting are rejected. Destructive
  owned edits/extraction reject shared or cyclic graphs received from native
  code, since freeing one branch could invalidate another.
- A native `Bfind` of VIEW returns a thread-local `BVIEWFLD` descriptor. Copy the
  descriptor before another native call can reuse it. Its data still borrows
  the source UBF.
- VIEW allocation requests may be rounded down to the compiled layout size.
  Copy the layout size, not the source allocation's spare capacity, and check
  the destination size before copying.
- Fast append uses `TypedUbf::fast_adder()` and its exclusive buffer borrow.
  There is no `TypedUbf::badd_fast` method; keep rustdoc links current.

## Structure mappings

These are custom `UbfSerialize`/`UbfDeserialize` and
`ViewSerialize`/`ViewDeserialize` traits, not generic `serde` trait implementations.

- `nested` selects inline UBF, `ptr` selects a UBF pointer, `view` selects inline
  VIEW, and `ptr, view` selects a VIEW pointer. Validate both native field kind
  and pointer target type. Deserialization does not extract ownership.
- `Vec<T>` means repeated occurrences; `UbfCarray(Vec<u8>)` means one binary
  field. A vector owns its suffix from the starting occurrence and trims stale
  entries. Arrays modify only their fixed window and preserve neighbors.
- A top-level `Option::None` clears its suffix. Optional elements inside repeated
  PTR/VIEW mappings instead occupy native NULL placeholders, preserving gaps
  and trailing NULLs. Inline UBF and scalar occurrences have no equivalent
  unambiguous NULL representation; wrap such optional values in a child struct.
- Check occurrence addition, signedness and native integer widths. Errors should
  identify the mapped struct/member. `default` applies only to an absent outer
  occurrence, not to malformed contents of a present child.
- Writes can partially update a buffer on error. Do not promise transactional
  serialization; callers can build a fresh buffer when needed.
- Flat `group` mappings use one parent occurrence column per row. Every member
  must consume one occurrence. Validate layout/offsets before modifying rows,
  including for empty collections. Use occurrence-preserving writes for nullable
  pointer/VIEW members.
- Group member offsets apply to writes, reads and suffix cleanup. Vector row
  counts exclude the anchor field's offset. Use checked arithmetic: plain
  `base + offset` previously caused overflow panics and incorrect cleanup
  previously deleted newly written rows.
- VIEW derives require a compiled layout name. Variable-length VIEW vectors
  require a `C` count indicator unless they fill the declared array; variable
  CARRAY lengths require `L`. Optional members use native NULL sentinels, and
  `Some(value)` equal to that sentinel must be rejected.
- The derive crate is a separate Cargo package. Test it separately, preserve
  generic/recursive types and renamed dependencies, and keep the root dependency
  version compatible with the macro APIs it uses.

## Async behavior

- Features `tokio` and `async-io` share `AsyncAtmiCtx` and imply `ctx-send`.
  Their futures are `!Send`; use a current-thread executor or Tokio `LocalSet`.
- Public adapter construction requires a native pollable reply queue.
  `build.rs` derives `endurox_pollable` from `ndrx_config.h` for EPOLL/KQUEUE.
  Other backends return `TPEINVAL`; enabling a Cargo feature cannot change the
  native backend. The local macOS setup used System V emulation during
  development; inspect the installed configuration before claiming coverage.
- Check the installed `tpext_getreplyqfd` implementation when validating KQUEUE:
  it must expose the real native queue descriptor. FreeBSD's native mqueue
  representation is different from macOS's emulated queue representation.
- Async `tpcall`, `tpacall`, and `tpgetrply` are awaited. `try_tpacall` performs
  one immediate attempt. Plain `AtmiCtx::tpacall` remains synchronous submission.
- Every async native send attempt uses `TPNOBLOCK`. Only send-side `TPEBLOCK`
  is retried, with async waits of 1, 2, 4, 8, then at most 16 ms. These are
  timer-based waits, not destination-writability notifications. Never replay a
  successful submission or retry arbitrary service/native errors.
- Preserve one original deadline across send waits and reply waits. Native
  `tpacall` consumes `TPBLK_NEXT` even on a failed send; retries temporarily apply
  remaining whole seconds, while the reactor retains the precise deadline.
  Restore another task's pending timeout before yielding. `TPNOTIME` disables
  the adapter deadline.
- Explicit `TPNOBLOCK` makes a full-queue send fail immediately. After a
  successful `tpcall` send, clear that flag for reply waiting. Explicit
  `tpgetrply(TPNOBLOCK)` still returns immediately when no reply is ready.
- Async and polled `tpcall` reject `TPNOREPLY`. `TPNOCHANGE` is checked when
  handing a parked reply to its caller, including the buffer subtype.
  `TPNOABORT` and `TPTRANSUSPEND` are rejected by the async demux because one
  `TPGETANY` drain cannot apply their receive semantics separately to each call.
- Native send attempts also consume priority on failure. Capture the resolved
  absolute priority before draining callbacks and reuse it on retries.
  `AsyncAtmiCtx::tpsprio` keeps the next async submission's priority in the
  adapter, separate from temporary native settings. Native operations use
  `context().tpsprio`; `into_inner` transfers an unused adapter priority back.
- One reply registration serves a context. Drain using `TPGETANY | TPNOBLOCK`
  and route by descriptor. Per-descriptor native collection can strand other
  replies in Enduro/X's memory queue, where fd readiness cannot wake their owner.
- Do not reuse a descriptor whose reply is still tracked by the demux. Preserve
  claim state so `TPGETANY` cannot steal a reply owned by `tpcall`.
- If native submission reuses an occupied descriptor, the existing policy
  cancels only the new generation and returns `TPELIMIT`, preserving the old
  parked reply. Cancellation drains again because native `tpcancel` can also
  move unrelated replies into the native memory queue. `TPGETANY` waiters each
  have their own registration so queue-level failures reach current waiters
  without leaking into later calls.
- Drivers duplicate the native reply fd, do not close the original, and do not
  change shared descriptor flags. Clear Tokio readiness only after a native
  receive reports would-block.
- Dropping a waiting send stops attempts. Dropping a submitted `tpcall` cancels
  its descriptor. Dropping `tpgetrply` leaves its caller-owned descriptor pending.

## Scripting plugins and Rust callbacks

- The Rust binding delegates to the core `tpscr*` API. The runtime selects its
  backend through `NDRX_PLUGINS`; Rust does not directly embed a particular
  interpreter. The discovery code in `tests/08_tpscript` is test setup, not a
  replacement for the core plugin loader.
- `AtmiCtx::tpscrinit(None, flags)` returns a `ScriptVm` borrowing that context.
  The VM exposes compile/load/unload/execute, callback registration and script
  error operations. `tpscruninit` consumes it; Drop also shuts it down.
  `NDRX_TPSCR_PACKAGE`, `NDRX_TPSCR_REPLACE` and `NDRX_TPSCR_FLAT` are exported.
- The portable core declares `tpscr_cfg_t` as opaque. Use `None` for defaults;
  `ScriptConfig::from_raw` is an unsafe backend-specific escape hatch. Do not
  invent a Rust configuration layout or assume the Python backend's treatment
  of configuration applies to every engine.
- `tpscrcomp` returns `ScriptBytecode`. Its allocation belongs to the plugin
  and must be freed with `tpscrfree`, not `tpfree` or Rust's allocator. Bytecode
  can outlive the VM while its borrowed `AtmiCtx` remains alive.
- Core callback registration already supports userdata:
  `tpscrregcb(vm, name, callback, void *user, flags)`. The callback typedef in
  `include/expluginbase.h` receives the same pointer. Rust uses this for a stable
  boxed callback entry containing the closure and its captured state.
- Registration userdata and execution context have different lifetimes. The
  callback obtains the ATMI context currently executing the script through
  `ScriptCallbackContext::context()`. Do not substitute a registration-time
  context when determining the current native context. No additional core
  userdata parameter is needed for the current binding; per-execution userdata
  would be a separate feature for arbitrary request-specific state.
- In default mode, the callback context is a borrowed view of current native
  TLS. With `ctx-send`, `AtmiCtx::borrow_current_context` temporarily detaches
  the invoking native `TPCONTEXT_T` into a borrowed Object API handle. Rust
  `ctx.*` operations use that same handle, and Drop restores native TLS before
  returning to the engine, including after a caught panic. The callback does
  not get an independent context; it cannot terminate or free the borrowed one.
- Callbacks are synchronous and local to the VM's creating thread. Rust closures
  may capture owned state, including `Rc` state; borrowed callback contexts and
  buffers cannot escape their invocation. The trampoline rejects callbacks on
  another thread before accessing the closure. VMs are not `Send`/`Sync`.
- Catch panics before they cross the C ABI and report failure through
  `tpscrseterror`. Preserve native error numbers when converting Rust ATMI errors
  into script errors. Snapshot `tperrno` before querying `tpscrerrno`/`tpscrerror`,
  since those public C entry points can clear it.
- Callback userdata addresses must remain stable across registry growth and
  VM moves. Keep entries alive until native shutdown, including registration
  failure paths where the provider may have partially installed the pointer.
  A failed shutdown retains userdata to avoid dangling provider references;
  query thread-local script error state rather than a possibly consumed VM.
- `ScriptBuffers` manages the parameter, input and output as references to unique
  allocations. `alias` shares an allocation; `take` clears every alias before
  returning ownership. Returning an input as output must never create a second
  owner. Adopt native pointer/length changes on failures as well as successes.
- `edit` and `edit_ubf` restore aliases after growth, errors and panics. Callback
  edits write through to the provider's real pointer slots before reentry:
  Python UBF wrappers may refer directly to those slots. Delaying writeback
  until callback exit can leave nested execution using a stale allocation.
- Distinguish equal buffer addresses from identical native pointer slots.
  Callback parameter/input slots can be the same `char **`; updates to such
  linked slots must agree. The trampoline validates this when returning buffers.
- A safe Rust callback can replace its entire `ScriptBuffers` value. Dropping
  the old collection must clear provider slots before freeing allocations;
  returning the new collection must transfer ownership back to C before its
  Rust temporary is dropped. Test this explicitly to prevent use-after-free.
- Native UBF lengths after growth can describe allocation capacity. Byte views
  expose only CARRAY payloads and initialized, terminated STRING/JSON contents.
  UBF/VIEW keep Rust's tracked byte length at zero; use typed access instead of
  exposing padding or unused capacity.

## Testing and documentation

For runtime/API changes, the maintainer's verification matrix includes:

```sh
cargo check --all-targets --all-features
cargo clippy --all-targets
cargo clippy --all-targets --features ctx-send
cargo clippy --all-targets --features tokio
cargo clippy --all-targets --features async-io
cargo clippy --all-targets --all-features
cargo test
cargo test --all-features
RUSTFLAGS='--cfg endurox_pollable' cargo check --all-targets --all-features
cargo fmt --all --check
git diff HEAD --check
```

Focused checks and documentation validation include:

```sh
cargo test --lib --all-features
cargo test --test ubf_tests --test atmictx_tests
cargo test --test ubf_tests --all-features
cargo test --features ctx-send
cargo test ubf_change_and_get_scalar_fields
cargo test --manifest-path endurox-rs-derive/Cargo.toml
cargo doc --no-deps
cargo doc --no-deps --all-features
```

For documentation-only changes, check content, links and whitespace; native
domain runs are not needed. Report which checks ran and what remains untested.

- Native tests require real Enduro/X libraries and IPC; they may need execution
  outside the sandbox. Do not interpret IPC permission errors as code failures.
- `tests/00_unittest/serde_complex.rs` is included by `ubf_tests`, not a standalone
  Cargo target. Reuse its `endurox_test_env()` guard and existing VIEW/field tables.
- Root integration tests launch separate crates under numbered `tests/`
  directories. Check those manifests explicitly when changing client/server
  sources; the root's `--all-targets` does not compile all nested crate binaries.
  Likewise, root formatting does not cover them: use
  `cargo fmt --manifest-path tests/<suite>/Cargo.toml --all --check` for each.
- Forced `endurox_pollable` compilation checks otherwise hidden branches only.
  It does not make a System V installation support reply-fd integration. Generic
  demux/timer tests and the send-only fixture still run on this backend; the
  live reply-reactor scenario needs Linux EPOLL or FreeBSD KQUEUE validation.
- Domain tests share `/test1` and IPC key 44000. Use
  `tests/common/endurox_domain_lock.rs`, which combines a process-local mutex
  with a file lock. Do not run standalone launchers concurrently against this
  domain or an active workload using it. Launchers provision config, clean test
  logs, and start/stop real processes.
- `cargo test --test 05_async_demux_it` includes a native send-pressure fixture
  through `run.sh --send-only`. Its ignored library test is invoked by the
  fixture with the proper environment. It uses a socket for an unused reply
  registration, so it proves real queue-send behavior on non-pollable backends
  without proving native reply-reactor integration. Report that distinction.
- Exercise both default and `ctx-send` paths for native-wrapper changes.
  Async changes should cover cancellation, deadline/priority preservation,
  descriptor routing and both runtime drivers as appropriate.
- Run `bash -n` for changed launchers. Check rustdoc after public API changes;
  compilation/tests alone do not catch broken intra-doc links. Format the
  proc-macro and nested test crates with their own `--manifest-path` when needed.
- Before handing off a staged implementation, check staged-tree completeness.
  Export the index with `git checkout-index` to a fresh temporary directory and
  build that tree, including nested test crates, with the same native discovery
  environment. Set `ENDUROX_MKFLDHDR` explicitly if the export lacks the sibling
  executable. A working-tree build can hide files missing from the index.

The main integration suites are:

| Suite | Coverage |
| --- | --- |
| `01_server_api` | RPC, forwarding, embedded UBF, OAPI and dispatch worker lifecycle |
| `02_server_extensions` | Poller, periodic and before-poll callbacks on the main context |
| `03_basic_carray_call` | CARRAY request/reply payloads |
| `05_async_demux` | Async reply routing and native send pressure/priority |
| `07_basic_durable_queue` | Durable queue operations and transactions |
| `08_tpscript` | Installed Python scripting plugin, default/OAPI callbacks, bytecode, buffer aliases and errors |

The scripting fixture honors `ENDUROX_TPSCRIPT_PLUGIN`, probes installed Python
packages (including user site packages) with interpreters on `PATH`, then tries
sibling build/source binaries. It logs the selected plugin; source-tree binaries
can be stale even when a newer user installation exists. It tests missing-plugin
errors in separate processes and reports skipped language coverage explicitly.

- `tests/08_tpscript/find_plugin.py` probes the default or `PYTHON`-selected
  interpreter, then available versioned Python commands. Use package discovery
  without importing the extension, which can initialize native middleware as a
  side effect. Match the extension ABI to the interpreter doing the discovery.
- On the development Mac, default `python3` was Xcode Python 3.9 while the working
  plugin was in the Python 3.14 user site under
  `~/Library/Python/3.14/lib/python/site-packages/endurox/`. The Homebrew system
  installation and sibling source directory contained older shared objects.
  These are observations, not fixed search paths or guarantees; rediscover and
  print the actual selected shared object before diagnosing backend behavior.
- `cargo test --test 08_tpscript_it -- --nocapture` runs the scripting fixture
  in default and `ctx-send` modes. It covers missing-plugin errors, bytecode,
  packages/replacement, captured callback state, nested execution, panic/error
  propagation, aliases, reallocation and whole-container replacement.
  `tests/00_unittest/script_buffer_tests.rs` adds plugin-independent ownership
  checks through the `ubf_tests` target.

## Working tree discipline

- Preserve staged and unstaged user changes, including release version edits.
  Inspect both `git diff` and `git diff --cached`; do not assume they match.
- When staging is requested, use explicit relevant paths and inspect
  `git diff --cached --check` and the staged file list. Do not blanket-stage
  unrelated local notes or generated files. Include changed build scripts,
  wrapper headers, manifests, lockfile and derive-crate files when applicable;
  `git add -u src tests` misses these and all new files.
- Build directories, compiled VIEW files, generated headers, logs and nested
  test lockfiles are artifacts. A standalone derive-crate test can also create
  an untracked `endurox-rs-derive/Cargo.lock`; do not confuse it with the tracked
  root lockfile.
- Keep changes focused, document unsafe ownership assumptions, and add
  regressions for observable failures rather than tests that merely repeat
  implementation details.
