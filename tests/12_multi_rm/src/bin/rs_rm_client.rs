#[path = "../../../common/test_duration.rs"]
mod test_duration;
use std::time::Instant;

use endurox_rs::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    ctx.tpopen2(0)?;
    assert_eq!(ctx.tprmopen("bad\0name", 0).unwrap_err().code, TPEINVAL);
    let request = ctx.tpalloc_carray(b"manual RM")?;
    let mut response = ctx.tpalloc_carray(&[])?;
    let duration = test_duration::test_duration();
    let started = Instant::now();
    let mut iterations = 0u64;
    while iterations < 2 || iterations % 2 != 0 || started.elapsed() < duration {
        let iteration = iterations;
        ctx.tprmopen("B", 0)?;
        let a = unsafe { ctx.tpgetconnrm(1)? };
        let b = unsafe { ctx.tpgetconnrm(2)? };
        assert_ne!(a, b);
        assert!(unsafe { ctx.tpgetconnrm(-1) }.is_err());
        ctx.tpbegin(60, 0)?;
        ctx.tprmstart("B", 0)?;
        ctx.tprmend("B", 0)?;
        ctx.tpcall("RSRM", &request, &mut response, 0)?;
        assert_eq!(response.as_bytes(), request.as_bytes());
        if iteration % 2 == 0 {
            ctx.tpcommit(0)?;
        } else {
            ctx.tpabort(0)?;
        }
        ctx.tprmclose("B", 0)?;
        iterations += 1;
    }
    println!(
        "Completed {} iterations in {:.3}s",
        iterations,
        started.elapsed().as_secs_f64()
    );
    ctx.tpclose()?;
    ctx.tpterm()?;
    Ok(())
}
