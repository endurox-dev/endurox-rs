//! Tokio scripting with native resources confined to a blocking worker.
//! Configure Enduro/X and the Python provider in NDRX_PLUGINS before running:
//! cargo run --features tokio --example scripting_tokio
use endurox_rs::{AtmiCtx, TpScrBuffers, TpScrError, TpScrResult, TpScrSlot, NDRX_TPSCR_FLAT};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let request = b"hello".to_vec();
        let reply = tokio::task::spawn_blocking(move || -> TpScrResult<Vec<u8>> {
            // These non-Send resources are created and released on this worker.
            let ctx = AtmiCtx::new()?;
            ctx.tpinit()?;
            let mut vm = ctx.tpscrinit(None, 0)?;
            vm.tpscrloadstr(
                "async_example",
                r#"def main(name, param, incoming, flags):
    return {'buftype': 'CARRAY', 'data': incoming['data'].upper() + b' from Python'}
"#,
                NDRX_TPSCR_FLAT,
            )?;

            let mut buffers = TpScrBuffers::new(&ctx);
            buffers.set(TpScrSlot::Input, Some(ctx.tpalloc_carray(&request)?))?;
            vm.tpscrexec("async_example", &mut buffers, 0)?;
            let reply = buffers
                .get(TpScrSlot::Output)?
                .ok_or_else(|| TpScrError::new(-1, "script returned no output"))?
                .as_bytes()
                .to_vec();

            drop(buffers);
            vm.tpscruninit(0)?;
            Ok(reply) // Only owned Rust bytes cross back to the async task.
        })
        .await??;

        println!("{}", String::from_utf8(reply)?);
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}
