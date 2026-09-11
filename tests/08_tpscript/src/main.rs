use endurox_rs::{
    ubf_fields as f, AtmiCtx, AtmiError, TpScrBuffers, TpScrError, TpScrResult, TpScrSlot as S,
    TpScrVm, TypedUbf, NDRX_TPSCR_FLAT, NDRX_TPSCR_PACKAGE, NDRX_TPSCR_REPLACE,
};
use std::cell::Cell;
use std::rc::Rc;

fn load(vm: &mut TpScrVm<'_>, name: &str, body: &str) -> TpScrResult<()> {
    vm.tpscrloadstr(
        name,
        format!(
            "FIELD = {}\ndef main(name, param, incoming, flags):\n    {}\n",
            f::T_LONG_FLD,
            body
        ),
        NDRX_TPSCR_FLAT,
    )
}

fn run() -> TpScrResult<()> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    if std::env::args().any(|arg| arg == "noplugin") {
        let error = match ctx.tpscrinit(None, 0) {
            Ok(_) => panic!("scripting unexpectedly initialized without a plugin"),
            Err(error) => error,
        };
        assert_eq!(error.atmi_code, AtmiError::TPERELEASE);
        println!("no-plugin error propagated");
        return Ok(());
    }
    let mut vm = ctx.tpscrinit(None, 0)?;
    let mut buffers = TpScrBuffers::new(&ctx);
    vm.tpscrseterror(1234, "manual error")?;
    assert_eq!(vm.tpscrerrno(), 1234);
    assert_eq!(vm.tpscrerror(), "manual error");

    let source = "def main(name, param, incoming, flags):\n    return {'buftype': 'CARRAY', 'data': b'compiled\\x00bytes'}\n";
    let bytes = vm.tpscrcomp(source, 0)?;
    assert!(!bytes.as_bytes().is_empty());
    vm.tpscrload("compiled", &bytes, NDRX_TPSCR_FLAT)?;
    let serialized = bytes.as_bytes().to_vec();
    bytes.tpscrfree()?;
    vm.tpscrexec("compiled", &mut buffers, 0)?;
    assert_eq!(
        buffers.get(S::Output)?.unwrap().as_bytes(),
        b"compiled\0bytes"
    );
    vm.tpscrunload("compiled", 0)?;
    assert!(vm.tpscrexec("compiled", &mut buffers, 0).is_err());
    assert_eq!(
        buffers.get(S::Output)?.unwrap().as_bytes(),
        b"compiled\0bytes"
    );
    vm.tpscrload("compiled", &serialized, NDRX_TPSCR_FLAT)?;
    assert!(vm.tpscrcomp("invalid python !", 0).is_err());
    assert!(vm.tpscrloadstr("bad\0name", "", 0).is_err());

    load(
        &mut vm,
        "string_result",
        "return {'buftype':'STRING','data':'string-result'}",
    )?;
    vm.tpscrexec("string_result", &mut buffers, 0)?;
    assert_eq!(
        buffers.get(S::Output)?.unwrap().as_bytes(),
        b"string-result\0"
    );

    // Source loading, packages and replacement travel through the same core API.
    vm.tpscrloadstr("rust_script_pkg", "VALUE = 7\n", NDRX_TPSCR_PACKAGE)?;
    vm.tpscrloadstr("rust_script_pkg.child", "from rust_script_pkg import VALUE\ndef main(name,param,incoming,flags):\n    return {'buftype':'CARRAY','data':bytes([VALUE])}\n", 0)?;
    vm.tpscrexec("rust_script_pkg.child", &mut buffers, 0)?;
    assert_eq!(buffers.get(S::Output)?.unwrap().as_bytes(), &[7]);
    vm.tpscrloadstr("rust_script_pkg.child", source, NDRX_TPSCR_REPLACE)?;
    vm.tpscrexec("rust_script_pkg.child", &mut buffers, 0)?;
    assert_eq!(
        buffers.get(S::Output)?.unwrap().as_bytes(),
        b"compiled\0bytes"
    );

    let calls = Rc::new(Cell::new(0));
    let callback_calls = calls.clone();
    vm.tpscrregcb(
        "host",
        move |call, args| {
            callback_calls.set(callback_calls.get() + 1);
            assert_eq!(call.name(), "host");
            assert_eq!(call.flags(), 37);
            assert_eq!(args.get(S::Input)?.unwrap().as_bytes(), b"from-python");
            args.set(
                S::Output,
                Some(call.context().tpalloc_carray(b"from-rust")?),
            )
        },
        0,
    )?;
    load(
        &mut vm,
        "callback",
        "return host(None, {'buftype':'CARRAY','data':b'from-python'}, 37)",
    )?;
    vm.tpscrexec("callback", &mut buffers, 0)?;
    assert_eq!(calls.get(), 1);
    assert_eq!(buffers.get(S::Output)?.unwrap().as_bytes(), b"from-rust");
    vm.tpscrregcb(
        "host",
        |call, args| {
            args.set(
                S::Output,
                Some(call.context().tpalloc_carray(b"replacement")?),
            )
        },
        0,
    )?;
    vm.tpscrexec("callback", &mut buffers, 0)?;
    assert_eq!(calls.get(), 1);
    assert_eq!(
        Rc::strong_count(&calls),
        1,
        "replacement retained old captures"
    );
    assert_eq!(buffers.get(S::Output)?.unwrap().as_bytes(), b"replacement");
    vm.tpscrunregcb("host", 0)?;
    assert!(vm.tpscrexec("callback", &mut buffers, 0).is_err());

    vm.tpscrregcb(
        "fail",
        |call, args| {
            args.set(
                S::Output,
                Some(call.context().tpalloc_carray(b"failure-output")?),
            )?;
            Err(TpScrError::new(7777, "Rust callback error"))
        },
        0,
    )?;
    load(&mut vm, "failure", "rc, output = fail(None, None, 0)\n    return {'rc':rc,'out':output,'screrrno':tpscrerrno(),'error':tpscrerror()}")?;
    let error = vm.tpscrexec("failure", &mut buffers, 0).unwrap_err();
    assert_eq!(error.script_code, 7777);
    assert!(error.message.contains("Rust callback error"));
    assert_eq!(
        buffers.get(S::Output)?.unwrap().as_bytes(),
        b"failure-output"
    );

    vm.tpscrregcb(
        "native_fail",
        |_, _| Err(AtmiError::new(AtmiError::TPEINVAL, "native callback failure").into()),
        0,
    )?;
    load(
        &mut vm,
        "native_failure",
        "return native_fail(None, None, 0)",
    )?;
    assert_eq!(
        vm.tpscrexec("native_failure", &mut buffers, 0)
            .unwrap_err()
            .script_code,
        AtmiError::TPEINVAL as i32
    );

    // Shared input/parameter/output, script-driven reallocations and error paths.
    let mut ubf = ctx.tpalloc_ubf(128)?;
    ubf.bchg(f::T_LONG_FLD, 0, 42i64, true)?;
    buffers.set(S::Input, Some(ubf.into_inner()))?;
    buffers.alias(S::Param, S::Input)?;
    buffers.alias(S::Output, S::Input)?;
    load(&mut vm, "echo", "return incoming")?;
    vm.tpscrexec("echo", &mut buffers, 0)?;
    let output = buffers.take(S::Output)?.unwrap();
    assert!(buffers.get(S::Input)?.is_none());
    assert!(buffers.get(S::Param)?.is_none());
    assert_eq!(
        TypedUbf::from_typed(output)?.bget_long(f::T_LONG_FLD, 0)?,
        42
    );

    let ubf = ctx.tpalloc_ubf(128)?;
    buffers.set(S::Input, Some(ubf.into_inner()))?;
    buffers.alias(S::Param, S::Input)?;
    buffers.set(S::Output, Some(ctx.tpalloc_carray(b"old-output")?))?;
    load(
        &mut vm,
        "grow_fail",
        &format!(
            "incoming['data'][{}] = 'x' * 32768\n    raise RuntimeError('after growth')",
            f::T_STRING_FLD
        ),
    )?;
    assert!(vm.tpscrexec("grow_fail", &mut buffers, 0).is_err());
    assert!(buffers.get(S::Input)?.unwrap().as_bytes().is_empty());
    assert_eq!(buffers.get(S::Output)?.unwrap().as_bytes(), b"old-output");
    buffers.edit_ubf(S::Input, |ubf| {
        assert_eq!(ubf.bget_string(f::T_STRING_FLD, 0)?.len(), 32768);
        Ok(())
    })?;

    // Callback relocation must update provider slots before nested execution.
    vm.tpscrregcb(
        "grow",
        |call, args| {
            args.edit_ubf(S::Input, |ubf| {
                ubf.tprealloc(131072)?;
                ubf.bchg(f::T_LONG_FLD, 0, 999i64, true)?;
                Ok(())
            })?;
            call.tpscrexec("echo", args, 0)
        },
        0,
    )?;
    load(&mut vm, "nested", "rc, output = grow(incoming, incoming, flags)\n    if rc != 0: return rc\n    assert incoming['data'][FIELD][0] == 999\n    return output")?;
    vm.tpscrexec("nested", &mut buffers, 0)?;
    assert!(buffers.get(S::Input)?.unwrap().tptypes()?.size >= 131072);
    assert_eq!(
        TypedUbf::from_typed(buffers.take(S::Output)?.unwrap())?.bget_long(f::T_LONG_FLD, 0)?,
        999
    );
    assert!(buffers.get(S::Input)?.is_none());

    vm.tpscrregcb(
        "panic_cb",
        |_, args| {
            args.edit_ubf(S::Input, |ubf| -> TpScrResult<()> {
                ubf.tprealloc(65536)?;
                ubf.bchg(f::T_LONG_FLD, 0, 123i64, true)?;
                panic!("intentional Rust callback panic");
            })
        },
        0,
    )?;
    buffers.set(S::Input, Some(ctx.tpalloc_ubf(128)?.into_inner()))?;
    load(
        &mut vm,
        "panic_case",
        "return panic_cb(None, incoming, flags)",
    )?;
    let error = vm.tpscrexec("panic_case", &mut buffers, 0).unwrap_err();
    assert!(error.message.contains("panicked"));
    buffers.edit_ubf(S::Input, |ubf| {
        assert_eq!(ubf.bget_long(f::T_LONG_FLD, 0)?, 123);
        Ok(())
    })?;
    vm.tpscrexec("echo", &mut buffers, 0)?; // VM and TLS remain usable.

    vm.tpscrregcb(
        "resize_bytes",
        |_, args| {
            args.edit(S::Input, |buffer| {
                buffer.set_bytes(&vec![9; 32768])?;
                Ok(())
            })?;
            args.alias(S::Output, S::Input)
        },
        0,
    )?;
    load(
        &mut vm,
        "bytes_callback",
        "return resize_bytes(None, {'buftype':'CARRAY','data':b'small'}, flags)",
    )?;
    vm.tpscrexec("bytes_callback", &mut buffers, 0)?;
    assert_eq!(buffers.get(S::Output)?.unwrap().as_bytes(), vec![9; 32768]);

    // Replacing the whole Rust container must hand the new allocation back to
    // the provider, rather than free it as the trampoline unwinds its locals.
    vm.tpscrregcb(
        "replace_args",
        |call, args| {
            *args = TpScrBuffers::new(call.context());
            args.set(
                S::Output,
                Some(call.context().tpalloc_carray(b"whole-container")?),
            )
        },
        0,
    )?;
    load(
        &mut vm,
        "replace_args_case",
        "return replace_args(None, {'buftype':'CARRAY','data':b'old'}, flags)",
    )?;
    vm.tpscrexec("replace_args_case", &mut buffers, 0)?;
    assert_eq!(
        buffers.get(S::Output)?.unwrap().as_bytes(),
        b"whole-container"
    );

    let retained = Rc::new(Cell::new(0));
    let capture = retained.clone();
    vm.tpscrregcb(
        "retained",
        move |_, _| {
            capture.set(1);
            Ok(())
        },
        0,
    )?;
    assert_eq!(Rc::strong_count(&retained), 2);
    vm.tpscruninit(0)?;
    assert_eq!(Rc::strong_count(&retained), 1);
    drop(buffers);
    // Automatic bytecode and VM destruction.
    let mut vm = ctx.tpscrinit(None, 0)?;
    let _bytes = vm.tpscrcomp(source, 0)?;
    drop(vm);
    drop(_bytes);
    println!("scripting lifecycle, Rust callbacks, errors, aliases and reallocation passed");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("TESTERROR: {error}");
        std::process::exit(1);
    }
}
