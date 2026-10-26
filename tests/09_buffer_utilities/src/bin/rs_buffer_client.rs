#[path = "../../../common/test_duration.rs"]
mod test_duration;
use std::time::Instant;

use endurox_rs::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    let bytes: Vec<u8> = (0..131073).map(|n| (n % 251) as u8).collect();
    let request = ctx.tpalloc_carray(&bytes)?;
    let mut reply = ctx.tpalloc_carray(&[])?;
    let low = ctx.tpalloc_carray(&[0])?;
    let high = ctx.tpalloc_carray(&[255])?;
    let duration = test_duration::test_duration();
    let started = Instant::now();
    let mut iterations = 0u64;
    while iterations < 1 || started.elapsed() < duration {
        assert_eq!(ctx.tpbufcmp(Some(&low), Some(&high), 0)?, -1);
        assert_eq!(ctx.tpbufcmp(Some(&high), Some(&low), 0)?, 1);
        assert_eq!(ctx.tpbufcmp(None, None, 0)?, 0);
        assert_eq!(
            ctx.tpbufcmp(Some(&low), Some(&high), 1).unwrap_err().code,
            TPEINVAL
        );
        for flags in [0, TPEX_STRING] {
            let exported = ctx.tpexport(&request, flags)?;
            assert!(exported.len() > 65536);
            let imported = ctx.tpimport(&exported, flags)?;
            assert_eq!(imported.as_bytes(), bytes);
        }
        ctx.tpcall("RSBUFFER", &request, &mut reply, 0)?;
        assert_eq!(reply.as_bytes(), bytes);
        iterations += 1;
    }
    println!(
        "Completed {} iterations in {:.3}s",
        iterations,
        started.elapsed().as_secs_f64()
    );
    match ctx.tpext_getreplyqfd() {
        Ok(fd) => assert!(fd >= 0),
        Err(e) => {
            assert_eq!(e.code, TPEINVAL);
            println!("Reply FD unavailable on this native backend");
        }
    }
    ctx.tpterm()?;
    Ok(())
}
