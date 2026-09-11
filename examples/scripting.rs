//! Requires a configured Enduro/X environment and its Python scripting plugin
//! in NDRX_PLUGINS, with the corresponding endurox Python package importable.
use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrResult, TpScrSlot as S, NDRX_TPSCR_FLAT};

fn main() -> TpScrResult<()> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    let mut vm = ctx.tpscrinit(None, 0)?;
    vm.tpscrregcb(
        "host",
        |call, args| {
            let mut bytes = args
                .get(S::Input)?
                .map(|buffer| buffer.as_bytes().to_vec())
                .unwrap_or_default();
            bytes.extend_from_slice(b" from Rust");
            args.set(S::Output, Some(call.context().tpalloc_carray(&bytes)?))
        },
        0,
    )?;
    vm.tpscrloadstr(
        "example",
        "def main(name, param, incoming, flags):\n    return host(param, incoming, flags)\n",
        NDRX_TPSCR_FLAT,
    )?;
    let mut buffers = TpScrBuffers::new(&ctx);
    buffers.set(S::Input, Some(ctx.tpalloc_carray(b"hello")?))?;
    vm.tpscrexec("example", &mut buffers, 0)?;
    let output = buffers.take(S::Output)?.unwrap();
    println!("{}", String::from_utf8_lossy(output.as_bytes()));
    Ok(())
}
