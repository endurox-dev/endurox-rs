use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use endurox_rs::{
    AtmiCtx, TpQCtl, TypedBuffer, TypedUbf, UbfValue, TPQCORRID, TPQFAILUREQ, TPQGETBYCORRID,
    TPQMSGID, TPQPRIORITY, TPQREPLYQ,
};

#[test]
fn atmictx_init_integration() {
    let _guard = endurox_test_env();

    // new() now returns Result<Self, AtmiError>
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    // tpinit() returns AtmiResult<()>
    ctx.tpinit().expect("tpinit failed");

    endurox_rs::ndrx_error!(ctx, "Context created...");

    // tpterm() returns AtmiResult<()>
    ctx.tpterm().expect("tpterm failed");
}

#[cfg(feature = "ctx-send")]
#[test]
fn ctx_send_context_can_move_to_another_thread() {
    fn assert_send<T: Send>() {}
    assert_send::<AtmiCtx>();

    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create detached AtmiCtx");
    std::thread::spawn(move || {
        ctx.tpinit().expect("Object API tpinit failed after move");
        let buffer = ctx
            .tpalloc_carray(b"moved context")
            .expect("Object API tpalloc failed after move");
        drop(buffer);
        ctx.tpterm().expect("Object API tpterm failed after move");
    })
    .join()
    .expect("moved context thread panicked");
}

#[cfg(all(feature = "ctx-send", feature = "async-io"))]
#[test]
fn async_io_adapter_preserves_movable_context_type() {
    fn assert_send<T: Send>() {}
    assert_send::<endurox_rs::AsyncIoAtmiCtx>();
}

#[test]
fn tpalloc_generic_and_cast_to_ubf() {
    let _guard = endurox_test_env();

    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    // generic typed buffer
    let tbuf: TypedBuffer<'_> = ctx.tpalloc("UBF", "", 0).expect("tpalloc failed");

    // "inherit" by casting to TypedUbf
    let mut ubf: TypedUbf<'_> = TypedUbf::from_typed(tbuf).expect("buffer is not UBF");

    assert!(ubf.bsizeof().expect("Bsizeof failed") > 0);

    //ctx.tpterm().expect("tpterm failed");
    ctx.tpinit().expect("Second init shall go OK");
}

#[test]
fn tpalloc_ubf() {
    let _guard = endurox_test_env();

    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    endurox_rs::ndrx_error!(ctx, ">>>>> About to alloc UBF...");
    let mut buf = ctx.tpalloc_ubf(1025).expect("Shall Alloc buffer OK");

    buf.bchg(1, 0, UbfValue::Long(5), false)
        .expect("Bchg failed");

    endurox_rs::ndrx_error!(ctx, ">>>>> About to free UBF...");
    drop(buf);
    drop(ctx);
}

#[cfg(all(feature = "async", not(endurox_pollable)))]
#[test]
fn async_adapters_report_non_pollable_endurox_backend() {
    let _guard = endurox_test_env();
    assert!(!AtmiCtx::ASYNC_SUPPORTED);

    #[cfg(feature = "tokio")]
    {
        assert!(!AtmiCtx::TOKIO_ASYNC_SUPPORTED);
        let ctx = AtmiCtx::new().expect("failed to create Tokio AtmiCtx");
        ctx.tpinit().expect("Tokio context tpinit failed");
        let error = match ctx.into_tokio() {
            Ok(_) => panic!("non-pollable Enduro/X backend accepted Tokio adapter"),
            Err(error) => error,
        };
        assert_eq!(error.code, endurox_rs::AtmiError::TPEINVAL);
        assert!(error.message.contains("EX_USE_EPOLL"));
    }

    #[cfg(feature = "async-io")]
    {
        let ctx = AtmiCtx::new().expect("failed to create async-io AtmiCtx");
        ctx.tpinit().expect("async-io context tpinit failed");
        let error = match ctx.into_async_io() {
            Ok(_) => panic!("non-pollable Enduro/X backend accepted async-io adapter"),
            Err(error) => error,
        };
        assert_eq!(error.code, endurox_rs::AtmiError::TPEINVAL);
        assert!(error.message.contains("EX_USE_EPOLL"));
    }
}

#[test]
fn buffer_length_cannot_exceed_allocation() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut buf = ctx
        .tpalloc_carray(&vec![0u8; 4096])
        .expect("tpalloc_carray failed");
    assert_eq!(buf.as_bytes().len(), 4096);

    // Shrinking must truncate the recorded length with it.
    buf.tprealloc(8).expect("tprealloc failed");
    assert!(
        buf.len() <= 8,
        "recorded length {} survived a shrink to 8 bytes",
        buf.len()
    );
    assert!(buf.as_bytes().len() <= 8);

    // And an oversized length must be refused outright.
    let err = buf
        .set_len(4096)
        .expect_err("set_len past the allocation should fail");
    assert_eq!(err.code, endurox_rs::AtmiError::TPEINVAL);
    assert!(buf.as_bytes().len() <= 8);

    drop(buf);
    ctx.tpterm().expect("tpterm failed");
}

/// Raw byte mutation is confined to CARRAY.
///
/// `set_bytes` over a UBF used to accept a serialised buffer straight from
/// `bwrite`, reproducing its `BFLD_PTR` occurrences as bare addresses that the
/// parent would later free. `set_len` over one zeroed the UBF header. Neither
/// buffer type has a plain byte payload, so both operations are refused.
#[test]
fn raw_byte_mutation_is_confined_to_carray() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    ubf.bchg(
        endurox_rs::ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("KEEP".to_string()),
        true,
    )
    .expect("bchg failed");

    let generic: &mut TypedBuffer<'_> = &mut ubf;
    for err in [
        generic
            .set_bytes(&[0u8; 16])
            .expect_err("set_bytes on a UBF must fail"),
        generic.set_len(8).expect_err("set_len on a UBF must fail"),
        generic
            .as_bytes_mut()
            .expect_err("as_bytes_mut on a UBF must fail"),
    ] {
        assert_eq!(err.code, endurox_rs::AtmiError::TPEINVAL);
    }

    // The buffer is intact: neither call reached the header.
    assert_eq!(
        ubf.bget_string(endurox_rs::ubf_fields::T_STRING_FLD, 0)
            .expect("the UBF header must be untouched"),
        "KEEP"
    );

    // CARRAY still works, and round-trips.
    let mut carray = ctx.tpalloc_carray(b"hello").expect("tpalloc_carray failed");
    carray.set_bytes(b"world!").expect("set_bytes on a CARRAY");
    assert_eq!(carray.as_bytes(), b"world!");
    carray.as_bytes_mut().expect("as_bytes_mut on a CARRAY")[0] = b'W';
    assert_eq!(carray.as_bytes(), b"World!");

    drop(carray);
    drop(ubf);
    ctx.tpterm().expect("tpterm failed");
}

/// A CARRAY's allocation is not its payload length, and extraction must not
/// pretend otherwise.
///
/// Reporting the allocation resurrected bytes the owner had truncated away and
/// exposed whatever sat past the data, so a 5-byte payload in a grown buffer
/// came back as the full allocation.
#[test]
fn extraction_does_not_mistake_capacity_for_payload() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    let mut target = ctx
        .tpalloc_carray(b"helloSECRET")
        .expect("tpalloc_carray failed");
    // The owner truncates to "hello" before handing the buffer over.
    target.set_len(5).expect("set_len failed");
    assert_eq!(target.as_bytes(), b"hello");
    // And the allocation is then grown well past the payload.
    target.tprealloc(4096).expect("tprealloc failed");

    master
        .bchg(
            endurox_rs::ubf_fields::T_PTR_FLD,
            0,
            UbfValue::Ptr(target),
            true,
        )
        .expect("storing a BFLD_PTR field failed");

    // Neither borrow nor extraction may invent a length.
    let borrowed = master
        .bget_ptr(endurox_rs::ubf_fields::T_PTR_FLD, 0)
        .expect("bget_ptr failed");
    assert_eq!(
        borrowed.as_bytes().len(),
        0,
        "a BFLD_PTR field stores an address, not an extent"
    );
    drop(borrowed);

    let mut extracted = master
        .bextract_ptr(endurox_rs::ubf_fields::T_PTR_FLD, 0)
        .expect("bextract_ptr failed");
    assert_eq!(extracted.as_bytes().len(), 0);

    // Stating the real length recovers the data.
    extracted.set_len(5).expect("set_len failed");
    assert_eq!(extracted.as_bytes(), b"hello");

    // Everything the caller can reach inside the allocation is initialised:
    // the bytes the growth added are zeroed rather than left as whatever the
    // allocator returned. (The original 11-byte payload is still physically
    // present below that; `set_len` narrows the view, it does not scrub.)
    extracted.set_len(4096).expect("set_len failed");
    assert_eq!(&extracted.as_bytes()[11..], &vec![0u8; 4085][..]);

    drop(extracted);
    drop(master);
    ctx.tpterm().expect("tpterm failed");
}

/// A failed receive must not erase the length of the buffer it was given.
///
/// `olen` is an in/out parameter. Seeding it with zero meant an error path
/// where Enduro/X returns before touching the buffer reported zero bytes,
/// making the caller's existing reply vanish from view.
#[test]
fn failed_receive_preserves_the_existing_reply() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut reply = ctx.tpalloc_carray(b"KEEP").expect("tpalloc_carray failed");
    assert_eq!(reply.as_bytes(), b"KEEP");

    // An invalid descriptor fails before the output buffer is touched.
    let mut revent: i64 = 0;
    let err = ctx
        .tprecv(-1, &mut reply, 0, &mut revent)
        .expect_err("tprecv on an invalid descriptor should fail");
    assert_ne!(err.code, 0);

    assert_eq!(
        reply.as_bytes(),
        b"KEEP",
        "a failed receive discarded the existing reply length"
    );

    // Same contract for tpcall against a service that does not exist.
    let req = ctx.tpalloc_carray(b"REQ").expect("tpalloc_carray failed");
    let _ = ctx.tpcall("RS_NO_SUCH_SVC", &req, &mut reply, 0);
    assert_eq!(
        reply.as_bytes(),
        b"KEEP",
        "a failed tpcall discarded the existing reply length"
    );

    drop(req);
    drop(reply);
    ctx.tpterm().expect("tpterm failed");
}

/// An allocation length that will not fit a `c_long` must be refused.
///
/// `tpalloc` takes a signed length. `usize::MAX` wrapped to -1, which Enduro/X
/// reads as below the CARRAY minimum and satisfies with a small buffer, so the
/// caller got a tiny allocation while the zeroing still ran for the size it
/// asked for -- a write far past the end of the buffer.
#[test]
fn oversized_allocations_are_refused() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let err = ctx
        .tpalloc("CARRAY", "", usize::MAX)
        .expect_err("an unrepresentable size must be refused");
    assert_eq!(err.code, endurox_rs::AtmiError::TPEINVAL);

    let err = ctx
        .tpalloc_ubf(usize::MAX)
        .expect_err("an unrepresentable size must be refused");
    assert_eq!(err.code, endurox_rs::AtmiError::TPEINVAL);

    // And on the realloc path.
    let mut buf = ctx.tpalloc_carray(b"x").expect("tpalloc_carray failed");
    let err = buf
        .tprealloc(usize::MAX)
        .expect_err("an unrepresentable size must be refused");
    assert_eq!(err.code, endurox_rs::AtmiError::TPEINVAL);
    assert_eq!(
        buf.as_bytes(),
        b"x",
        "a refused realloc must change nothing"
    );

    drop(buf);
    ctx.tpterm().expect("tpterm failed");
}

/// X_OCTET is Enduro/X's alias for CARRAY and must get the same treatment.
///
/// The clearing used to key off the type string the caller passed, so a buffer
/// allocated as X_OCTET skipped it and `set_len` then exposed the uninitialised
/// allocation. The allocator's own minimum size had the same effect: asking for
/// fewer bytes than `CARRAY_DEFAULT_SIZE` returns a larger buffer whose tail
/// was never cleared.
#[test]
fn carray_aliases_and_the_allocator_minimum_are_initialised() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    const SIZE: usize = 4096;

    // Leave recycled memory behind with a recognisable pattern in it.
    for _ in 0..32 {
        let mut dirty = ctx
            .tpalloc("CARRAY", "", SIZE)
            .expect("tpalloc CARRAY failed");
        dirty.set_len(SIZE).expect("set_len failed");
        dirty
            .as_bytes_mut()
            .expect("as_bytes_mut failed")
            .fill(0xAA);
    }

    // The alias must report as CARRAY and must be just as clean.
    for round in 0..32 {
        let mut buf = ctx
            .tpalloc("X_OCTET", "", SIZE)
            .expect("tpalloc X_OCTET failed");
        assert_eq!(
            buf.tptypes().expect("tptypes failed").type_name,
            "CARRAY",
            "X_OCTET is an alias for CARRAY"
        );
        buf.set_len(SIZE).expect("set_len failed");
        assert_eq!(
            buf.as_bytes(),
            &vec![0u8; SIZE][..],
            "round {round}: an X_OCTET allocation kept a previous buffer's contents"
        );
    }

    // A request under the allocator's minimum returns a larger buffer; every
    // byte the caller can reach in it must still be initialised.
    for round in 0..32 {
        let mut buf = ctx.tpalloc("CARRAY", "", 1).expect("tpalloc failed");
        let allocated = buf.tptypes().expect("tptypes failed").size;
        buf.set_len(allocated).expect("set_len failed");
        assert_eq!(
            buf.as_bytes(),
            &vec![0u8; allocated][..],
            "round {round}: the tail past the requested size was left dirty"
        );
    }

    ctx.tpterm().expect("tpterm failed");
}

/// A round trip through tpexport/tpimport must keep the payload readable.
///
/// tpimport discarded the length Enduro/X reported, so the imported buffer came
/// back reporting zero bytes and stating the real length used to overwrite the
/// payload with nulls.
#[test]
fn import_preserves_the_decoded_length() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let source = ctx.tpalloc_carray(b"hello").expect("tpalloc_carray failed");
    let exported = ctx.tpexport(&source, 0).expect("tpexport failed");

    let mut imported = ctx.tpimport(&exported, 0).expect("tpimport failed");
    assert_eq!(
        imported.as_bytes(),
        b"hello",
        "tpimport dropped the length Enduro/X reported"
    );

    // And restating that length must leave the payload alone.
    imported.set_len(5).expect("set_len failed");
    assert_eq!(imported.as_bytes(), b"hello");

    drop(imported);
    drop(source);
    ctx.tpterm().expect("tpterm failed");
}

/// A CARRAY allocation must never expose uninitialised memory.
///
/// `set_len` no longer zeroes the range it exposes, because doing so destroyed
/// live data. The guarantee moved to allocation time instead: `tpalloc` and a
/// growing `tprealloc` clear what they hand out, so every byte inside the
/// allocation is initialised whatever length the caller states.
#[test]
fn carray_allocations_are_never_uninitialised() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    // Dirty a batch of blocks and release them, so the allocator has recycled
    // memory with a recognisable pattern to hand back. A freshly mapped page
    // reads as zero anyway, which would let this pass without the clearing.
    const SIZE: usize = 4096;
    for _ in 0..32 {
        let mut dirty = ctx
            .tpalloc("CARRAY", "", SIZE)
            .expect("tpalloc CARRAY failed");
        dirty.set_len(SIZE).expect("set_len failed");
        dirty
            .as_bytes_mut()
            .expect("as_bytes_mut failed")
            .fill(0xAA);
    }

    // No allocation may show any of it.
    for round in 0..32 {
        let mut buf = ctx
            .tpalloc("CARRAY", "", SIZE)
            .expect("tpalloc CARRAY failed");
        buf.set_len(SIZE).expect("set_len failed");
        assert_eq!(
            buf.as_bytes(),
            &vec![0u8; SIZE][..],
            "round {round}: tpalloc handed back memory a previous buffer had written"
        );
    }

    let mut buf = ctx
        .tpalloc("CARRAY", "", SIZE)
        .expect("tpalloc CARRAY failed");

    // Growth must clear the newly added tail too.
    buf.set_bytes(b"data").expect("set_bytes failed");
    buf.tprealloc(8192).expect("tprealloc failed");
    buf.set_len(8192).expect("set_len failed");
    assert_eq!(&buf.as_bytes()[..4], b"data");
    assert_eq!(&buf.as_bytes()[4..], &vec![0u8; 8188][..]);

    drop(buf);
    ctx.tpterm().expect("tpterm failed");
}

/// Stating the length of an extracted `BFLD_PTR` target must not destroy it.
///
/// `set_len` used to zero the range it newly exposed, so a caller recording the
/// length the payload already had got nulls back instead of the data.
#[test]
fn extracted_carray_keeps_its_payload() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    let target = ctx.tpalloc_carray(b"hello").expect("tpalloc_carray failed");
    master
        .bchg(
            endurox_rs::ubf_fields::T_PTR_FLD,
            0,
            UbfValue::Ptr(target),
            true,
        )
        .expect("storing a BFLD_PTR field failed");

    // A BFLD_PTR field stores an address and no extent, so the borrow starts
    // empty rather than guessing from the allocation.
    assert_eq!(
        master
            .bget_ptr(endurox_rs::ubf_fields::T_PTR_FLD, 0)
            .expect("bget_ptr failed")
            .as_bytes()
            .len(),
        0
    );

    let mut extracted = master
        .bextract_ptr(endurox_rs::ubf_fields::T_PTR_FLD, 0)
        .expect("bextract_ptr failed");
    assert_eq!(extracted.len(), 0);

    // Stating the length the data already has must hand it back untouched.
    extracted.set_len(5).expect("set_len failed");
    assert_eq!(extracted.as_bytes(), b"hello");

    drop(extracted);
    drop(master);
    ctx.tpterm().expect("tpterm failed");
}

/// TPEX_STRING must be refused on the byte API, and the string API must
/// round-trip.
///
/// In string mode Enduro/X calls `ndrx_crypto_enc_string(input, output, olen)`,
/// which takes no input length and reads to the first NUL. A `&[u8]` gives no
/// such guarantee, so the native side would read past the slice.
#[test]
fn encrypt_string_mode_is_separated_from_the_byte_api() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    // Deliberately unterminated bytes plus the string flag.
    let err = ctx
        .tpencrypt(b"no terminator here", endurox_rs::TPEX_STRING)
        .expect_err("TPEX_STRING must be rejected by the byte API");
    assert_eq!(err.code, endurox_rs::AtmiError::TPEINVAL);

    let err = ctx
        .tpdecrypt(b"no terminator here", endurox_rs::TPEX_STRING)
        .expect_err("TPEX_STRING must be rejected by the byte API");
    assert_eq!(err.code, endurox_rs::AtmiError::TPEINVAL);

    // The string API handles termination itself and round-trips.
    let secret = "round trip me";
    let encrypted = ctx
        .tpencrypt_string(secret)
        .expect("tpencrypt_string failed");
    assert_ne!(encrypted, secret);
    let decrypted = ctx
        .tpdecrypt_string(&encrypted)
        .expect("tpdecrypt_string failed");
    assert_eq!(decrypted, secret);
}

#[test]
fn tpqctl_sets_flags_and_bounded_fields() {
    let mut qctl = TpQCtl::default();

    qctl.set_flags(TPQCORRID | TPQPRIORITY)
        .add_flags(TPQREPLYQ | TPQFAILUREQ | TPQMSGID)
        .clear_flags(TPQPRIORITY);
    assert_eq!(qctl.flags(), TPQCORRID | TPQREPLYQ | TPQFAILUREQ | TPQMSGID);

    qctl.set_corrid(b"ORDER-1001").expect("set corrid failed");
    qctl.set_msgid(b"MSG-1").expect("set msgid failed");
    qctl.set_reply_queue("REPLYQ")
        .expect("set reply queue failed");
    qctl.set_failure_queue("ERRORQ")
        .expect("set failure queue failed");
    qctl.set_priority(50)
        .set_deq_time(30)
        .set_delivery_qos(2)
        .set_reply_qos(4)
        .set_exp_time(60)
        .set_urcode(7)
        .set_appkey(9);

    assert_eq!(qctl.corrid(), b"ORDER-1001");
    assert_eq!(qctl.msgid(), b"MSG-1");
    assert_eq!(qctl.reply_queue(), "REPLYQ");
    assert_eq!(qctl.failure_queue(), "ERRORQ");
    assert_eq!(qctl.priority(), 50);
    assert_eq!(qctl.deq_time(), 30);
    assert_eq!(qctl.delivery_qos(), 2);
    assert_eq!(qctl.reply_qos(), 4);
    assert_eq!(qctl.exp_time(), 60);
    assert_eq!(qctl.urcode(), 7);
    assert_eq!(qctl.appkey(), 9);
    assert_eq!(qctl.diagnostic(), 0);
    assert_eq!(qctl.diagmsg(), "");

    qctl.set_flags(TPQGETBYCORRID);
    assert_eq!(qctl.flags(), TPQGETBYCORRID);

    assert!(qctl.set_corrid(&[b'x'; 32]).is_err());
    assert!(qctl.set_reply_queue("1234567890123456").is_err());
    assert!(qctl.set_failure_queue("bad\0queue").is_err());
}

fn endurox_test_env() -> MutexGuard<'static, ()> {
    let guard = match endurox_test_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    provision_endurox_env();
    guard
}

fn endurox_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn provision_endurox_env() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let test_dir = manifest_dir.join("tests").join("00_unittest");
        let ubf_file = manifest_dir.join("tests").join("ubftab").join("test.fd");

        let output = Command::new("bash")
            .arg("-lc")
            .arg(
                r#"
set -euo pipefail
cd "$NDRX_RS_UNIT_TEST_DIR"
if [ -f "$HOME/ndrx_home" ]; then
    . "$HOME/ndrx_home"
fi
rm -f conf/app.ini conf/settest1
mkdir -p log
find log -type f -exec rm -f {} +
xadmin provision -d -vaddubf="$NDRX_RS_UNIT_UBF_FILE" >/dev/null
. conf/settest1
unset NDRX_DEBUG_CONF
export NDRX_DEBUG_STR="file=$NDRX_RS_UNIT_TEST_DIR/log/atmi-tests.log ndrx=5"
env
"#,
            )
            .env("NDRX_RS_UNIT_TEST_DIR", &test_dir)
            .env("NDRX_RS_UNIT_UBF_FILE", &ubf_file)
            .output()
            .expect("failed to run xadmin provision for atmictx tests");

        if !output.status.success() {
            panic!(
                "xadmin provision failed with status={}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some((key, value)) = line.split_once('=') {
                env::set_var(key, value);
            }
        }
    });
}
