use endurox_rs::{
    ubf_fields, AtmiCtx, AtmiResult, PollerEvent, TpReturnStatus, TpSvcInfo, UbfValue,
};

static mut READ_FD: i32 = -1;
static mut WRITE_FD: i32 = -1;
static mut B4POLL_COUNT: usize = 0;
static mut POLLER_COUNT: usize = 0;
static mut WRITE_PENDING: bool = false;
static mut INSTALLED: bool = false;

fn rs_ext_b4poll_cb() -> i32 {
    unsafe {
        B4POLL_COUNT += 1;

        if !WRITE_PENDING {
            WRITE_PENDING = true;
            if WRITE_FD >= 0 {
                let byte = [1_u8; 1];
                let _ = libc::write(WRITE_FD, byte.as_ptr().cast(), byte.len());
            }
        }
    }

    0
}

fn rs_ext_poller_cb(_event: PollerEvent) -> i32 {
    unsafe {
        POLLER_COUNT += 1;

        if READ_FD >= 0 {
            let mut byte = [0_u8; 1];
            let _ = libc::read(READ_FD, byte.as_mut_ptr().cast(), byte.len());
        }
        WRITE_PENDING = false;
    }

    0
}

fn rs_ext_install(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };

    let should_install = unsafe {
        let was_installed = INSTALLED;
        INSTALLED = true;
        !was_installed
    };

    if should_install {
        let read_fd = unsafe { READ_FD };
        if ctx
            .tpext_addpollerfd(read_fd, libc::POLLIN as u32, 0, rs_ext_poller_cb)
            .and_then(|_| ctx.tpext_addb4pollcb(rs_ext_b4poll_cb))
            .is_err()
        {
            unsafe {
                INSTALLED = false;
            }
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 1, ubf, 0);
            return;
        }
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

fn rs_ext_status(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };

    let (b4poll, poller) = unsafe { (B4POLL_COUNT, POLLER_COUNT) };
    let ok = b4poll > 0 && poller > 0;

    let should_uninstall = unsafe {
        let was_installed = INSTALLED;
        if ok {
            INSTALLED = false;
        }
        ok && was_installed
    };

    if should_uninstall {
        let _ = ctx.tpext_delb4pollcb();
        let read_fd = unsafe { READ_FD };
        let _ = ctx.tpext_delpollerfd(read_fd);
    }

    let rsp = format!("b4poll={b4poll};poller={poller};ok={ok}");

    if ubf
        .bchg(ubf_fields::T_STRING_2_FLD, 0, UbfValue::String(rsp), true)
        .is_err()
    {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 2, ubf, 0);
        return;
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

fn rs_ext_init(ctx: &AtmiCtx) -> AtmiResult<()> {
    let mut fds = [-1; 2];
    let pipe_rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if pipe_rc != 0 {
        return Err(ctx.atmi_last_error());
    }

    unsafe {
        READ_FD = fds[0];
        WRITE_FD = fds[1];
        B4POLL_COUNT = 0;
        POLLER_COUNT = 0;
        WRITE_PENDING = false;
        INSTALLED = false;
    }

    ctx.tpadvertise("RS_EXT_INSTALL", rs_ext_install)?;
    ctx.tpadvertise("RS_EXT_STATUS", rs_ext_status)?;

    Ok(())
}

fn rs_ext_done(_ctx: &AtmiCtx) {
    let read_fd = unsafe {
        let fd = READ_FD;
        READ_FD = -1;
        fd
    };
    if read_fd >= 0 {
        let _ = unsafe { libc::close(read_fd) };
    }

    let write_fd = unsafe {
        let fd = WRITE_FD;
        WRITE_FD = -1;
        fd
    };
    if write_fd >= 0 {
        let _ = unsafe { libc::close(write_fd) };
    }
}

fn main() {
    let ctx = match AtmiCtx::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to create context: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = ctx.tp_run(rs_ext_init, rs_ext_done) {
        eprintln!("extension server failed: {e}");
        std::process::exit(1);
    }
}
