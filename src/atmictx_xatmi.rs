//! XATMI calls, conversations, transactions, events, queues, and timeout configuration.
use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TpTranId, TypedBuffer, TypedUbf};
use core::ffi::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::ptr;
use std::time::{Duration, Instant};

#[cfg(endurox_pollable)]
struct PendingCall<'ctx> {
    ctx: &'ctx AtmiCtx,
    cd: i32,
    armed: bool,
}

/// Cancellation guard for a synchronous call collected through the reply queue.
#[cfg(endurox_pollable)]
impl<'ctx> PendingCall<'ctx> {
    /// Arm a guard that cancels a call if its reply is not collected.
    ///
    /// # Arguments
    ///
    /// - `ctx`: Context that owns the pending call.
    /// - `cd`: Outstanding call descriptor returned by `tpacall`.
    fn new(ctx: &'ctx AtmiCtx, cd: i32) -> Self {
        Self {
            ctx,
            cd,
            armed: true,
        }
    }

    /// Disarm cancellation after the reply has been collected.
    fn complete(&mut self) {
        self.armed = false;
    }
}

/// Cancel the guarded call if reply collection did not complete.
#[cfg(endurox_pollable)]
impl Drop for PendingCall<'_> {
    /// Cancel the guarded call if reply collection did not complete.
    fn drop(&mut self) {
        if self.armed {
            let _ = self.ctx.tpcancel(self.cd);
        }
    }
}

/// Read a NUL-terminated string out of a byte buffer the C side filled.
///
/// # Arguments
///
/// - `bytes`: Native output bytes; decoding stops at the first NUL or the slice end.
fn cstr_prefix_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Request/reply messaging, transactions, queues, and native context settings.
impl AtmiCtx {
    /// Synchronous RPC call with separate input and output buffers.
    ///
    /// # Arguments
    ///
    /// - `svc`: Advertised destination service name, without embedded NUL bytes.
    /// - `idata`: Borrowed request buffer; the call uses its tracked payload length.
    /// - `odata`: Reply buffer, updated with native pointer and length changes even when the
    ///   call fails.
    /// - `flags`: Call flags such as `TPNOBLOCK`, `TPNOTIME`, or `TPNOCHANGE`; `TPNOREPLY` is
    ///   invalid.
    ///
    /// This mirrors the C API: `idata` is the request buffer, and `odata` is the
    /// reply buffer. Enduro/X may reallocate `odata`; this wrapper adopts the
    /// returned pointer and length on both success and failure.
    pub fn tpcall(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;
        let ilen = idata.len() as c_long;
        let mut reply = odata.as_ptr();
        // `olen` is in/out: it carries the current length in and the reply
        // length back. Seeding it with zero meant an error path where Enduro/X
        // returns before touching the buffer reported a length of zero, wiping
        // the caller's existing contents from view.
        let mut olen: c_long = odata.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpcall(
                c_svc.as_ptr() as *mut c_char,
                idata.as_ptr(),
                ilen,
                &mut reply,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpcall(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                idata.as_ptr(),
                ilen,
                &mut reply,
                &mut olen,
                flags as c_long,
            )
        };

        // Adopt the reply buffer on every path, as `tpgetrply` does. Enduro/X
        // runs `ndrx_mbuf_prepare_incoming(.., char **odata, long *olen, ..)`,
        // which can reallocate, *before* raising TPESVCFAIL. Keeping the old
        // pointer on that path leaves it dangling and leaks the replacement.
        odata.replace_ptr(reply);
        odata.set_len_reported(olen.max(0) as usize);

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Submit a request without waiting for its reply, returning a call descriptor.
    ///
    /// Submission itself is synchronous and may block unless `TPNOBLOCK` is set.
    /// Collect the reply with `tpgetrply`; `TPNOREPLY` returns zero after sending.
    ///
    /// # Arguments
    ///
    /// - `svc`: Advertised destination service name, without embedded NUL bytes.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: Submission flags; `TPNOBLOCK` fails on a full queue, and `TPNOREPLY` requests
    ///   no reply.
    pub fn tpacall(&self, svc: &str, data: &TypedBuffer<'_>, flags: i64) -> AtmiResult<i32> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;
        let ilen = data.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpacall(
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                ilen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpacall(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                ilen,
                flags as c_long,
            )
        };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Retrieve the reply for a previous `tpacall`.
    ///
    /// # Arguments
    ///
    /// - `cd`: Requested descriptor on input; updated to the received descriptor, including
    ///   with `TPGETANY`.
    /// - `data`: Reply buffer; native pointer and length updates are retained even on failure.
    /// - `flags`: Receive flags; `TPGETANY` selects any reply, and `TPNOBLOCK` returns
    ///   immediately if none is ready.
    ///
    /// `cd` is updated by the framework when `TPGETANY` is used.
    pub fn tpgetrply(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let mut c_cd = *cd as c_int;
        let mut odata = data.as_ptr();
        // `olen` is in/out: it carries the current length in and the reply
        // length back. Seeding it with zero meant an error path where Enduro/X
        // returns before touching the buffer reported a length of zero, wiping
        // the caller's existing contents from view.
        let mut olen: c_long = data.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetrply(&mut c_cd, &mut odata, &mut olen, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpgetrply(
                self.c_ctx_ptr(),
                &mut c_cd,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        // Adopt the descriptor and buffer on *every* path, not just on success.
        // `ndrx_tpgetrply` assigns `*cd = rply->cd` and runs
        // `ndrx_mbuf_prepare_incoming(.., char **odata, long *olen, ..)` -- which
        // may reallocate -- before it raises TPESVCFAIL or TPETIME. Keeping the
        // old pointer on those paths leaves it dangling and leaks the
        // replacement, and drops the descriptor that says which call failed.
        *cd = c_cd as i32;
        data.replace_ptr(odata);
        data.set_len_reported(olen.max(0) as usize);

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Cancel a pending asynchronous call descriptor returned by `tpacall`.
    ///
    /// # Arguments
    ///
    /// - `cd`: Outstanding call descriptor returned by `tpacall`.
    pub fn tpcancel(&self, cd: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcancel(cd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcancel(self.c_ctx_ptr(), cd as c_int) };

        self.rc_to_result(rc)
    }

    /// Deadline for one reply wait, taken from Enduro/X's own effective call
    /// timeout rather than from a caller-supplied duration.
    ///
    /// `tpgblktime(0)` resolves `tpsblktime(TPBLK_NEXT)`, then `tpsblktime`/
    /// `tptoutset` thread settings, then `NDRX_TOUT`. Read it **before**
    /// `tpacall`: a `TPBLK_NEXT` setting is one-shot and the call consumes it.
    ///
    /// The deadline matters beyond reporting `TPETIME`. Enduro/X only expires
    /// call descriptors inside `tpgetrply` (`call_scan_tout`), so a poll loop
    /// with no timer would never wake to let that run and would wait forever.
    pub(crate) fn reply_deadline(&self) -> AtmiResult<Option<Instant>> {
        let secs = self.tpgblktime(0)?;
        if secs <= 0 {
            return Ok(None);
        }
        Instant::now()
            .checked_add(Duration::from_secs(secs as u64))
            .ok_or_else(|| {
                AtmiError::new(raw::TPEINVAL, "Enduro/X call timeout exceeds Instant range")
            })
            .map(Some)
    }

    /// Synchronous call that uses the async/reply-queue path only on pollable
    /// Enduro/X builds.
    ///
    /// # Arguments
    ///
    /// - `svc`: Advertised destination service name, without embedded NUL bytes.
    /// - `idata`: Borrowed request buffer; the call uses its tracked payload length.
    /// - `odata`: Reply buffer, updated with native pointer and length changes even when the
    ///   call fails.
    /// - `flags`: Call flags such as `TPNOBLOCK`, `TPNOTIME`, or `TPNOCHANGE`; `TPNOREPLY` is
    ///   invalid.
    ///
    /// On `EX_USE_EPOLL` and `EX_USE_KQUEUE` builds this performs `tpacall`,
    /// waits for readiness on the internal reply queue descriptor, then drains
    /// the requested call descriptor with `tpgetrply(TPNOBLOCK)`. Other queue
    /// backends are not externally pollable, so this falls back to the normal
    /// blocking `tpcall` path (`Otpcall` when `ctx-send` is enabled).
    ///
    /// Timeouts come from `NDRX_TOUT` / `tptoutset` / `tpsblktime`, exactly as
    /// for [`AtmiCtx::tpcall`]. There is no per-call timeout argument.
    pub fn tpcall_polled(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(endurox_pollable))]
        {
            self.tpcall(svc, idata, odata, flags)
        }

        #[cfg(endurox_pollable)]
        {
            self.tpcall_pollable(svc, idata, odata, flags)
        }
    }

    /// Submit a request and collect its reply using the native queue descriptor and one deadline.
    ///
    /// # Arguments
    ///
    /// - `svc`: Advertised destination service name, without embedded NUL bytes.
    /// - `idata`: Borrowed request buffer; the call uses its tracked payload length.
    /// - `odata`: Reply buffer, updated with native pointer and length changes even when the
    ///   call fails.
    /// - `flags`: Call flags such as `TPNOBLOCK`, `TPNOTIME`, or `TPNOCHANGE`; `TPNOREPLY` is
    ///   invalid.
    ///
    #[cfg(endurox_pollable)]
    fn tpcall_pollable(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        if flags & raw::TPNOREPLY as i64 != 0 {
            // The public tpcall() rejects this (libatmi/atmi.c:330), so the
            // polled variant must too rather than silently returning without a
            // reply. tpacall would hand back descriptor 0, which can never be
            // collected.
            return Err(AtmiError::new(
                raw::TPEINVAL,
                "TPNOREPLY cannot be used with tpcall()",
            ));
        }

        // TPNOTIME disables Enduro/X's own call timeout, so imposing the
        // tpgblktime deadline here would cancel a call the caller asked to wait
        // on indefinitely.
        let deadline = if flags & raw::TPNOTIME as i64 != 0 {
            None
        } else {
            self.reply_deadline()?
        };
        let cd = self.tpacall(svc, idata, flags)?;
        let mut pending = PendingCall::new(self, cd);
        let result = self.tpgetrply_polled(&mut pending.cd, odata, flags, deadline);
        if result.is_ok() {
            pending.complete();
        }
        result
    }

    /// Collect a reply, blocking in `poll` between nonblocking native receive attempts.
    ///
    /// # Arguments
    ///
    /// - `cd`: Descriptor to collect, updated by the native receive call.
    /// - `data`: Reply buffer whose pointer and length may change on either success or failure.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    /// - `deadline`: Absolute end of the wait, or `None` to wait without an adapter deadline.
    ///
    #[cfg(endurox_pollable)]
    fn tpgetrply_polled(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
        deadline: Option<Instant>,
    ) -> AtmiResult<()> {
        let reply_fd = self.reply_queue_fd()?;
        let get_flags = flags | raw::TPNOBLOCK as i64;

        loop {
            match self.tpgetrply(cd, data, get_flags) {
                Ok(()) => return Ok(()),
                Err(err) if err.code == raw::TPEBLOCK => {}
                Err(err) => return Err(err),
            }

            if !self.poll_reply_queue(reply_fd, deadline)? {
                // The timer fired. Give Enduro/X one more chance to expire the
                // descriptor itself so the caller sees its bookkeeping, and only
                // synthesize TPETIME if it still reports nothing.
                match self.tpgetrply(cd, data, get_flags) {
                    Ok(()) => return Ok(()),
                    Err(err) if err.code == raw::TPEBLOCK => {
                        return Err(AtmiError::new(raw::TPETIME, "polled tpcall timed out"))
                    }
                    Err(err) => return Err(err),
                }
            }
        }
    }

    /// Borrow the native reply queue descriptor, or return `TPEINVAL` on an unsupported backend.
    pub(crate) fn reply_queue_fd(&self) -> AtmiResult<c_int> {
        #[cfg(not(endurox_pollable))]
        {
            Err(AtmiError::new(
                raw::TPEINVAL,
                "async integration requires an Enduro/X EX_USE_EPOLL or EX_USE_KQUEUE build",
            ))
        }

        #[cfg(endurox_pollable)]
        {
            #[cfg(not(feature = "ctx-send"))]
            let reply_fd = unsafe { raw::tpext_getreplyqfd() };

            #[cfg(feature = "ctx-send")]
            let reply_fd = unsafe { raw::Otpext_getreplyqfd(self.c_ctx_ptr()) };

            if reply_fd < 0 {
                Err(self.atmi_last_error())
            } else {
                Ok(reply_fd)
            }
        }
    }

    /// Wait for reply-queue readability; return `false` when the deadline expires.
    ///
    /// # Arguments
    ///
    /// - `reply_fd`: Borrowed native reply queue descriptor; this function does not close it.
    /// - `deadline`: Absolute end of the wait, or `None` to wait without an adapter deadline.
    ///
    #[cfg(endurox_pollable)]
    fn poll_reply_queue(&self, reply_fd: c_int, deadline: Option<Instant>) -> AtmiResult<bool> {
        let mut pfd = libc::pollfd {
            fd: reply_fd,
            events: libc::POLLIN,
            revents: 0,
        };

        loop {
            let timeout_ms = match deadline {
                Some(d) => d
                    .checked_duration_since(Instant::now())
                    .map(|remaining| {
                        let millis = remaining.as_millis();
                        if remaining.is_zero() {
                            0
                        } else {
                            millis.max(1).min(c_int::MAX as u128) as c_int
                        }
                    })
                    .unwrap_or(0),
                None => -1,
            };
            pfd.revents = 0;
            let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if rc > 0 {
                if (pfd.revents & libc::POLLIN) != 0 {
                    return Ok(true);
                }
                let error_events = libc::POLLERR | libc::POLLHUP | libc::POLLNVAL;
                if (pfd.revents & error_events) != 0 {
                    return Err(AtmiError::new(
                        raw::TPEOS,
                        format!(
                            "poll on Enduro/X reply queue returned events {:#x}",
                            pfd.revents
                        ),
                    ));
                }
                continue;
            }
            if rc == 0 {
                return Ok(false);
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(AtmiError::new(
                raw::TPEOS,
                format!("poll on Enduro/X reply queue failed: {err}"),
            ));
        }
    }

    /// Open a conversational connection.
    ///
    /// # Arguments
    ///
    /// - `svc`: Advertised destination service name, without embedded NUL bytes.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: Conversation flags, including `TPSENDONLY` or `TPRECVONLY` to select initial
    ///   control.
    ///
    /// The payload length comes from the buffer itself. An independent `len`
    /// argument bypassed the allocation checks on `set_len`, so a value larger
    /// than the buffer reached the native copy.
    pub fn tpconnect(&self, svc: &str, data: &TypedBuffer<'_>, flags: i64) -> AtmiResult<i32> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpconnect(
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpconnect(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Close a conversational connection and release its descriptor.
    ///
    /// # Arguments
    ///
    /// - `cd`: Conversation descriptor returned by `tpconnect` or supplied to a service.
    pub fn tpdiscon(&self, cd: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpdiscon(cd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpdiscon(self.c_ctx_ptr(), cd as c_int) };

        self.rc_to_result(rc)
    }

    /// Receive on a conversational connection.
    ///
    /// # Arguments
    ///
    /// - `cd`: Conversation descriptor to receive from.
    /// - `data`: Receive buffer; native pointer and length changes are retained even on failure.
    /// - `flags`: Receive options, including `TPNOBLOCK` for an immediate attempt.
    /// - `revent`: Output conversation event code, also updated when the operation returns
    ///   `TPEEVENT`.
    ///
    /// `revent` is an out-parameter, mirroring the C API, because the event is
    /// the whole point of the `TPEEVENT` error: returning it only on success
    /// would discard exactly the information the caller needs to react to a
    /// disconnect or a service completion.
    pub fn tprecv(
        &self,
        cd: i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
        revent: &mut i64,
    ) -> AtmiResult<usize> {
        let mut odata = data.as_ptr();
        // `olen` is in/out: it carries the current length in and the reply
        // length back. Seeding it with zero meant an error path where Enduro/X
        // returns before touching the buffer reported a length of zero, wiping
        // the caller's existing contents from view.
        let mut olen: c_long = data.len() as c_long;
        let mut c_revent: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tprecv(
                cd as c_int,
                &mut odata,
                &mut olen,
                flags as c_long,
                &mut c_revent,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otprecv(
                self.c_ctx_ptr(),
                cd as c_int,
                &mut odata,
                &mut olen,
                flags as c_long,
                &mut c_revent,
            )
        };

        // Same reasoning as tpcall/tpgetrply: the buffer may have been replaced
        // before the error was raised, and the length was previously dropped
        // even on success.
        *revent = c_revent as i64;
        data.replace_ptr(odata);
        data.set_len_reported(olen.max(0) as usize);

        if rc == raw::EXSUCCEED as c_int {
            Ok(olen.max(0) as usize)
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Send on a conversational connection.
    ///
    /// # Arguments
    ///
    /// - `cd`: Conversation descriptor to send on.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: Send options, including `TPRECVONLY` to hand control to the peer.
    /// - `revent`: Output conversation event code, also updated when the operation returns
    ///   `TPEEVENT`.
    ///
    /// `revent` is an out-parameter for the same reason as [`Self::tprecv`]:
    /// the event carries the meaning of a `TPEEVENT` failure and must survive
    /// the error return. The payload length comes from the buffer, not from a
    /// separate unchecked argument.
    pub fn tpsend(
        &self,
        cd: i32,
        data: &TypedBuffer<'_>,
        flags: i64,
        revent: &mut i64,
    ) -> AtmiResult<()> {
        let mut c_revent: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpsend(
                cd as c_int,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
                &mut c_revent,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpsend(
                self.c_ctx_ptr(),
                cd as c_int,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
                &mut c_revent,
            )
        };

        *revent = c_revent as i64;

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Roll back the current global transaction.
    ///
    /// # Arguments
    ///
    /// - `flags`: Reserved by the native abort API; pass `0`.
    pub fn tpabort(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpabort(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpabort(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Set whether transaction commit waits for completion or only for the commit decision to
    /// be logged.
    ///
    /// # Arguments
    ///
    /// - `flags`: Native `TP_CMT_COMPLETE` or `TP_CMT_LOGGED` commit-return mode.
    pub fn tpscmt(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpscmt(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpscmt(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Start a global transaction with a transaction timeout.
    ///
    /// # Arguments
    ///
    /// - `timeout`: Maximum transaction lifetime in seconds.
    /// - `flags`: Reserved by the native begin API; pass `0`.
    pub fn tpbegin(&self, timeout: u64, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpbegin(timeout as _, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpbegin(self.c_ctx_ptr(), timeout as _, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Commit the current global transaction using the configured commit mode.
    ///
    /// # Arguments
    ///
    /// - `flags`: `0` uses the configured commit mode; native `TPTXCOMMITDLOG` returns after
    ///   the commit decision is logged.
    pub fn tpcommit(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcommit(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcommit(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Suspend the current global transaction. Returns a `TpTranId` that can
    /// later be passed to `tpresume` to rejoin the transaction.
    ///
    /// # Arguments
    ///
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpsuspend(&self, flags: i64) -> AtmiResult<TpTranId> {
        let mut tranid: raw::TPTRANID = unsafe { std::mem::zeroed() };

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsuspend(&mut tranid, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsuspend(self.c_ctx_ptr(), &mut tranid, flags as c_long) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(TpTranId(tranid))
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Resume a previously suspended global transaction.
    ///
    /// # Arguments
    ///
    /// - `tranid`: Identifier returned when the transaction was suspended.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpresume(&self, tranid: &TpTranId, flags: i64) -> AtmiResult<()> {
        let mut inner = tranid.0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpresume(&mut inner, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpresume(self.c_ctx_ptr(), &mut inner, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Open the XA resource manager associated with this context.
    pub fn tpopen(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpopen() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpopen(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Close the XA resource manager associated with this context.
    pub fn tpclose(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpclose() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpclose(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Return the transaction level: zero outside a global transaction, one inside it.
    pub fn tpgetlev(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetlev() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetlev(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Query the native extended error-detail code.
    ///
    /// # Arguments
    ///
    /// - `flags`: Reserved by the native API; pass `0`.
    pub fn tperrordetail(&self, flags: i64) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tperrordetail(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otperrordetail(self.c_ctx_ptr(), flags as c_long) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Return explanatory text for an extended error-detail code.
    ///
    /// # Arguments
    ///
    /// - `err`: Native error-detail code to describe.
    /// - `flags`: Reserved by the native API; pass `0`.
    pub fn tpstrerrordetail(&self, err: i32, flags: i64) -> AtmiResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let ptr = unsafe { raw::tpstrerrordetail(err as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let ptr =
            unsafe { raw::Otpstrerrordetail(self.c_ctx_ptr(), err as c_int, flags as c_long) };

        if ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    /// Return the symbolic name of an ATMI error code, such as `TPEINVAL`.
    ///
    /// # Arguments
    ///
    /// - `err`: ATMI error number whose symbolic name is requested.
    pub fn tpecodestr(&self, err: i32) -> AtmiResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let ptr = unsafe { raw::tpecodestr(err as c_int) };

        #[cfg(feature = "ctx-send")]
        let ptr = unsafe { raw::Otpecodestr(self.c_ctx_ptr(), err as c_int) };

        if ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    /// Return the local Enduro/X node identifier.
    pub fn tpgetnodeid(&self) -> AtmiResult<i64> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetnodeid() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetnodeid(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_long {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i64)
        }
    }

    /// Register an event subscription and return its identifier.
    ///
    /// # Arguments
    ///
    /// - `eventexpr`: Regular expression matching event names to subscribe to.
    /// - `filter`: Optional buffer-specific filter expression; `None` disables payload filtering.
    /// - `ctl`: Event-delivery control block with a destination service in `name1`.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpsubscribe(
        &self,
        eventexpr: &str,
        filter: Option<&str>,
        ctl: &crate::TpEvCtl,
        flags: i64,
    ) -> AtmiResult<i64> {
        let c_expr = CString::new(eventexpr)
            .map_err(|_| AtmiError::new(AtmiError::TPEINVAL, "event expression contains NUL"))?;
        let c_filter = filter
            .map(CString::new)
            .transpose()
            .map_err(|_| AtmiError::new(AtmiError::TPEINVAL, "event filter contains NUL"))?;
        let filter_ptr = c_filter
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());
        let mut native_ctl = ctl.inner;
        let ctl_ptr = &mut native_ctl;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpsubscribe(
                c_expr.as_ptr() as *mut c_char,
                filter_ptr,
                ctl_ptr,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpsubscribe(
                self.c_ctx_ptr(),
                c_expr.as_ptr() as *mut c_char,
                filter_ptr,
                ctl_ptr,
                flags as c_long,
            )
        };

        if rc == raw::EXFAIL as c_long {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i64)
        }
    }

    /// Remove an event subscription from the event broker.
    ///
    /// # Arguments
    ///
    /// - `subscription`: Subscription identifier, or `-1` to remove this client’s subscriptions.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    ///
    /// Returns the number of subscriptions removed.
    pub fn tpunsubscribe(&self, subscription: i64, flags: i64) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpunsubscribe(subscription as c_long, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpunsubscribe(self.c_ctx_ptr(), subscription as c_long, flags as c_long)
        };

        if rc < 0 {
            Err(self.atmi_last_error())
        } else {
            Ok(rc)
        }
    }

    /// Publish an event and its payload to matching subscribers.
    ///
    /// # Arguments
    ///
    /// - `eventname`: Event name tested against registered subscription expressions.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    ///
    /// Returns the number of servers that consumed the event.
    pub fn tppost(&self, eventname: &str, data: &TypedBuffer<'_>, flags: i64) -> AtmiResult<i32> {
        let c_event = CString::new(eventname)
            .map_err(|_| AtmiError::new(AtmiError::TPEINVAL, "event name contains NUL"))?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tppost(
                c_event.as_ptr() as *mut c_char,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otppost(
                self.c_ctx_ptr(),
                c_event.as_ptr() as *mut c_char,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        if rc < 0 {
            Err(self.atmi_last_error())
        } else {
            Ok(rc)
        }
    }

    /// Initialize an application thread with no authentication (null TPINIT).
    pub fn tpappthrinit(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpappthrinit(ptr::null_mut()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpappthrinit(self.c_ctx_ptr(), ptr::null_mut()) };

        self.rc_to_result(rc)
    }

    /// Release the application-thread state initialized by `tpappthrinit`.
    pub fn tpappthrterm(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpappthrterm() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpappthrterm(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Check the native client-authentication requirement.
    ///
    /// This signature returns `Ok(())` for native `TPNOAUTH`; it does not expose an
    /// authentication-mode value.
    pub fn tpchkauth(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpchkauth() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpchkauth(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Send an unsolicited message to a specific client.
    ///
    /// # Arguments
    ///
    /// - `clientid`: Destination client identifier, usually obtained from `TpSvcInfo::cltid`.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpnotify(
        &self,
        clientid: &mut crate::ClientId,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpnotify(
                clientid,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpnotify(
                self.c_ctx_ptr(),
                clientid,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Broadcast an unsolicited message to matching clients.
    ///
    /// # Arguments
    ///
    /// - `lmid`: Optional logical machine identifier filter; `None` matches any machine.
    /// - `usrname`: Optional user-name filter; `None` matches any user.
    /// - `cltname`: Optional client-name filter; `None` matches any client name.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpbroadcast(
        &self,
        lmid: Option<&str>,
        usrname: Option<&str>,
        cltname: Option<&str>,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_lmid = lmid
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;
        let c_usr = usrname
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;
        let c_clt = cltname
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;

        let p_lmid = c_lmid
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());
        let p_usr = c_usr
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());
        let p_clt = c_clt
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpbroadcast(
                p_lmid,
                p_usr,
                p_clt,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpbroadcast(
                self.c_ctx_ptr(),
                p_lmid,
                p_usr,
                p_clt,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Ask Enduro/X to dispatch pending unsolicited messages.
    pub fn tpchkunsol(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpchkunsol() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpchkunsol(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Set the process-wide XATMI timeout in seconds. Call after initialization and serialize
    /// updates.
    ///
    /// # Arguments
    ///
    /// - `tout`: Positive process-wide timeout in seconds; context overrides can take precedence.
    pub fn tptoutset(&self, tout: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptoutset(tout as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptoutset(self.c_ctx_ptr(), tout as c_int) };

        self.rc_to_result(rc)
    }

    /// Return the process-wide XATMI timeout in seconds.
    pub fn tptoutget(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptoutget() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptoutget(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Decode an Enduro/X exported representation into a newly owned typed buffer.
    ///
    /// # Arguments
    ///
    /// - `payload`: Exported buffer bytes to decode.
    /// - `flags`: Import options; `0` reads JSON export data, and `TPEX_STRING` reads
    ///   base64-encoded export data.
    pub fn tpimport<'ctx>(&'ctx self, payload: &[u8], flags: i64) -> AtmiResult<TypedBuffer<'ctx>> {
        let mut obuf: *mut c_char = ptr::null_mut();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpimport(
                payload.as_ptr() as *mut c_char,
                payload.len() as c_long,
                &mut obuf,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpimport(
                self.c_ctx_ptr(),
                payload.as_ptr() as *mut c_char,
                payload.len() as c_long,
                &mut obuf,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            let mut buf = unsafe { TypedBuffer::from_raw(self, obuf) };
            // Enduro/X reports how much it decoded. Dropping it left the
            // imported payload invisible to `as_bytes`.
            buf.set_len_reported(olen.max(0) as usize);
            Ok(buf)
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Encode a typed buffer in Enduro/X export format using a 64 KiB output area.
    ///
    /// # Arguments
    ///
    /// - `ibuf`: Typed buffer to export; this wrapper passes zero as the native input length.
    /// - `flags`: `0` for JSON export data, or `TPEX_STRING` for its base64 encoding.
    pub fn tpexport(&self, ibuf: &TypedBuffer<'_>, flags: i64) -> AtmiResult<Vec<u8>> {
        let mut out = vec![0u8; 65536];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpexport(
                ibuf.as_ptr(),
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpexport(
                self.c_ctx_ptr(),
                ibuf.as_ptr(),
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            out.truncate(olen as usize);
            Ok(out)
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Return the resource manager’s native connection pointer without taking ownership.
    ///
    /// # Safety
    ///
    /// Any use of the returned pointer must obey the selected resource manager’s type,
    /// lifetime, and thread rules.
    pub(crate) unsafe fn tpgetconn(&self) -> *mut ::std::os::raw::c_void {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::tpgetconn()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Otpgetconn(self.c_ctx_ptr())
        }
    }

    /// Read call metadata attached to a message into a native UBF output buffer.
    ///
    /// # Arguments
    ///
    /// - `msg`: Valid native typed-buffer pointer whose call metadata is accessed.
    /// - `cibuf`: Writable UBF pointer slot; Enduro/X may allocate or replace the output buffer.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub(crate) fn tpgetcallinfo(
        &self,
        msg: *const c_char,
        cibuf: *mut *mut raw::UBFH,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetcallinfo(msg, cibuf, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetcallinfo(self.c_ctx_ptr(), msg, cibuf, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Attach call metadata from a native UBF buffer to a message.
    ///
    /// # Arguments
    ///
    /// - `msg`: Valid native typed-buffer pointer whose call metadata is accessed.
    /// - `cibuf`: Valid native UBF containing metadata to attach.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub(crate) fn tpsetcallinfo(
        &self,
        msg: *const c_char,
        cibuf: *mut raw::UBFH,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsetcallinfo(msg, cibuf, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsetcallinfo(self.c_ctx_ptr(), msg, cibuf, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Populate a UBF buffer from a JSON string.
    ///
    /// # Arguments
    ///
    /// - `ubf`: Destination UBF to populate; it must have enough free capacity.
    /// - `json`: JSON text in Enduro/X field format, without embedded NUL bytes.
    pub fn tpjsontoubf(&self, ubf: &mut TypedUbf<'_>, json: &str) -> AtmiResult<()> {
        use std::ffi::CString;
        let c_json = CString::new(json).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpjsontoubf(ubf.as_ubfh(), c_json.as_ptr() as *mut c_char) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpjsontoubf(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                c_json.as_ptr() as *mut c_char,
            )
        };

        self.rc_to_result(rc)
    }

    /// Serialize a UBF buffer to a JSON string.
    ///
    /// # Arguments
    ///
    /// - `ubf`: UBF buffer whose fields are converted to JSON.
    pub fn tpubftojson(&self, ubf: &TypedUbf<'_>) -> AtmiResult<String> {
        // Allocate a reasonably-sized output buffer; grow on first call if needed.
        let mut out = vec![0u8; 65536];

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpubftojson(
                ubf.as_ubfh(),
                out.as_mut_ptr() as *mut c_char,
                out.len() as c_int,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpubftojson(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                out.as_mut_ptr() as *mut c_char,
                out.len() as c_int,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            let end = out.iter().position(|&b| b == 0).unwrap_or(out.len());
            Ok(String::from_utf8_lossy(&out[..end]).into_owned())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Write a VIEW’s JSON representation into caller-provided native storage.
    ///
    /// # Arguments
    ///
    /// - `cstruct`: Readable native VIEW allocation matching the named layout.
    /// - `view`: Pointer to the NUL-terminated compiled VIEW name.
    /// - `buffer`: Writable output storage for JSON, including a trailing NUL.
    /// - `bufsize`: Capacity of the JSON output storage in bytes.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub(crate) fn tpviewtojson(
        &self,
        cstruct: *mut c_char,
        view: *mut c_char,
        buffer: *mut c_char,
        bufsize: i32,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc =
            unsafe { raw::tpviewtojson(cstruct, view, buffer, bufsize as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpviewtojson(
                self.c_ctx_ptr(),
                cstruct,
                view,
                buffer,
                bufsize as c_int,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Allocate a VIEW from JSON and write its compiled layout name to the output field.
    ///
    /// # Arguments
    ///
    /// - `view`: Writable native VIEW-name output field, with at least 34 bytes.
    /// - `buffer`: Readable NUL-terminated JSON text whose outer key names the VIEW layout.
    ///
    /// # Safety
    ///
    /// The name output must be writable for 34 bytes and the JSON input must be a readable C
    /// string. The returned allocation transfers to the caller.
    pub(crate) unsafe fn tpjsontoview(
        &self,
        view: *mut c_char,
        buffer: *mut c_char,
    ) -> AtmiResult<*mut c_char> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = raw::tpjsontoview(view, buffer);

        #[cfg(feature = "ctx-send")]
        let rc = raw::Otpjsontoview(self.c_ctx_ptr(), view, buffer);

        if rc.is_null() {
            Err(self.atmi_last_error())
        } else {
            Ok(rc)
        }
    }

    /// Enqueue a buffer into a persistent queue.
    ///
    /// # Arguments
    ///
    /// - `qspace`: Configured persistent queue-space name.
    /// - `qname`: Queue name within the selected queue space or server.
    /// - `ctl`: Queue options on input and message metadata or diagnostics on output, including
    ///   on failure.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpenqueue(
        &self,
        qspace: &str,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_qspace = CString::new(qspace).map_err(|_| self.atmi_last_error())?;
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpenqueue(
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpenqueue(
                self.c_ctx_ptr(),
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Dequeue a buffer from a persistent queue. Returns the dequeued buffer.
    ///
    /// # Arguments
    ///
    /// - `qspace`: Configured persistent queue-space name.
    /// - `qname`: Queue name within the selected queue space or server.
    /// - `ctl`: Queue options on input and message metadata or diagnostics on output, including
    ///   on failure.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpdequeue<'ctx>(
        &'ctx self,
        qspace: &str,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        flags: i64,
    ) -> AtmiResult<TypedBuffer<'ctx>> {
        let c_qspace = CString::new(qspace).map_err(|_| self.atmi_last_error())?;
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();
        let mut odata: *mut c_char = ptr::null_mut();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpdequeue(
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpdequeue(
                self.c_ctx_ptr(),
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            // Keep the reported length: dropping it surfaces every dequeued
            // CARRAY message as empty.
            let mut buf = unsafe { TypedBuffer::from_raw(self, odata) };
            buf.set_len_reported(olen.max(0) as usize);
            Ok(buf)
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Enqueue a payload using an explicit queue-server node and server identifier.
    ///
    /// # Arguments
    ///
    /// - `nodeid`: Enduro/X node hosting the queue server.
    /// - `srvid`: Server identifier of the queue server on that node.
    /// - `qname`: Queue name within the selected queue space or server.
    /// - `ctl`: Queue options on input and message metadata or diagnostics on output, including
    ///   on failure.
    /// - `data`: Borrowed payload to send; its tracked length supplies the native message length.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpenqueueex(
        &self,
        nodeid: i16,
        srvid: i16,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpenqueueex(
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpenqueueex(
                self.c_ctx_ptr(),
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                data.len() as c_long,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Dequeue a buffer by node/server ID. Returns the dequeued buffer.
    ///
    /// # Arguments
    ///
    /// - `nodeid`: Enduro/X node hosting the queue server.
    /// - `srvid`: Server identifier of the queue server on that node.
    /// - `qname`: Queue name within the selected queue space or server.
    /// - `ctl`: Queue options on input and message metadata or diagnostics on output, including
    ///   on failure.
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    pub fn tpdequeueex<'ctx>(
        &'ctx self,
        nodeid: i16,
        srvid: i16,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        flags: i64,
    ) -> AtmiResult<TypedBuffer<'ctx>> {
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();
        let mut odata: *mut c_char = ptr::null_mut();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpdequeueex(
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpdequeueex(
                self.c_ctx_ptr(),
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            // Keep the reported length: dropping it surfaces every dequeued
            // CARRAY message as empty.
            let mut buf = unsafe { TypedBuffer::from_raw(self, odata) };
            buf.set_len_reported(olen.max(0) as usize);
            Ok(buf)
        } else {
            Err(self.atmi_last_error())
        }
    }

    // `tpgetctxt` / `tpsetctxt` are deliberately absent from the safe API.
    //
    // `tpsetctxt` attaches a context to the calling thread and leaves it
    // attached. `AtmiCtx`'s `unsafe impl Send` rests on the opposite: the
    // handle is created detached (`tpnewctxt(0, 0)`) and the Object API
    // attaches it only for the duration of one call. A context left attached
    // has thread affinity while its Rust owner still claims to be `Send`, and
    // `tpsetctxt(TPNULLCONTEXT)` frees the attached context outright
    // (libatmi/atmi_tls.c) while the owner is still live.
    //
    // Nothing is lost. Moving work between threads is what the `ctx-send`
    // feature is for: it makes `AtmiCtx` itself `Send`, and every operation
    // attaches and detaches around its own call.

    /// Reject `TPEX_STRING` on the byte-slice APIs.
    ///
    /// # Arguments
    ///
    /// - `flags`: XATMI operation flags; use `0` for the native default behavior.
    /// - `what`: Operation name included in a validation error.
    ///
    /// In string mode Enduro/X calls `ndrx_crypto_enc_string(input, output,
    /// olen)`, which takes no input length and therefore reads to the first NUL.
    /// A `&[u8]` carries no such guarantee, so the native side would read past
    /// the slice. Use [`AtmiCtx::tpencrypt_string`] /
    /// [`AtmiCtx::tpdecrypt_string`] instead.
    fn reject_string_mode(flags: i64, what: &str) -> AtmiResult<()> {
        if flags & raw::TPEX_STRING as i64 != 0 {
            return Err(AtmiError::new(
                raw::TPEINVAL,
                format!(
                    "TPEX_STRING cannot be used with {what}: the native call reads \
                     the input to its first NUL, which a byte slice does not \
                     guarantee. Use {what}_string instead."
                ),
            ));
        }
        Ok(())
    }

    /// Encrypt a byte payload. `TPEX_STRING` is rejected; see
    /// [`AtmiCtx::tpencrypt_string`].
    ///
    /// # Arguments
    ///
    /// - `input`: Binary payload to encrypt.
    /// - `flags`: Native encryption options; pass `0` for binary mode. `TPEX_STRING` is rejected.
    pub fn tpencrypt(&self, input: &[u8], flags: i64) -> AtmiResult<Vec<u8>> {
        Self::reject_string_mode(flags, "tpencrypt")?;
        let mut out = vec![0u8; input.len().saturating_mul(2).max(256)];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpencrypt(
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpencrypt(
                self.c_ctx_ptr(),
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            out.truncate(olen as usize);
            Ok(out)
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Decrypt a byte payload. `TPEX_STRING` is rejected; see
    /// [`AtmiCtx::tpdecrypt_string`].
    ///
    /// # Arguments
    ///
    /// - `input`: Encrypted binary payload produced by the native encryption API.
    /// - `flags`: Native encryption options; pass `0` for binary mode. `TPEX_STRING` is rejected.
    pub fn tpdecrypt(&self, input: &[u8], flags: i64) -> AtmiResult<Vec<u8>> {
        Self::reject_string_mode(flags, "tpdecrypt")?;
        let mut out = vec![0u8; input.len().max(256)];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpdecrypt(
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpdecrypt(
                self.c_ctx_ptr(),
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            out.truncate(olen as usize);
            Ok(out)
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Encrypt a string with `TPEX_STRING`, producing base64 output.
    ///
    /// # Arguments
    ///
    /// - `input`: Plaintext without embedded NUL bytes.
    ///
    /// The native call reads the input to its first NUL and NUL-terminates the
    /// result, so both sides are handled as C strings here rather than as byte
    /// slices.
    pub fn tpencrypt_string(&self, input: &str) -> AtmiResult<String> {
        let c_input = CString::new(input)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "input contains a NUL byte"))?;
        // Base64 grows the payload; leave room for the terminator too.
        let mut out = vec![0u8; input.len().saturating_mul(2).max(256) + 1];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpencrypt(
                c_input.as_ptr() as *mut c_char,
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                raw::TPEX_STRING as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpencrypt(
                self.c_ctx_ptr(),
                c_input.as_ptr() as *mut c_char,
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                raw::TPEX_STRING as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(cstr_prefix_to_string(&out))
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Decrypt a base64 string produced by [`AtmiCtx::tpencrypt_string`].
    ///
    /// # Arguments
    ///
    /// - `input`: Base64 ciphertext produced by `tpencrypt_string`, without embedded NUL bytes.
    pub fn tpdecrypt_string(&self, input: &str) -> AtmiResult<String> {
        let c_input = CString::new(input)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "input contains a NUL byte"))?;
        let mut out = vec![0u8; input.len().max(256) + 1];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpdecrypt(
                c_input.as_ptr() as *mut c_char,
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                raw::TPEX_STRING as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpdecrypt(
                self.c_ctx_ptr(),
                c_input.as_ptr() as *mut c_char,
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                raw::TPEX_STRING as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(cstr_prefix_to_string(&out))
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Set the priority for the next native message submission.
    ///
    /// # Arguments
    ///
    /// - `prio`: Absolute priority from 1 to 100, or a relative adjustment from -100 to 100.
    /// - `flags`: `TPABSOLUTE` to use an absolute priority, or `0` for a service-relative
    ///   adjustment.
    pub fn tpsprio(&self, prio: i32, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsprio(prio as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsprio(self.c_ctx_ptr(), prio as c_int, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Return the resolved priority of the last native message submission.
    pub fn tpgprio(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgprio() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgprio(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Set a context-specific timeout for the next applicable call or for all calls.
    ///
    /// # Arguments
    ///
    /// - `tout`: Nonnegative timeout in seconds; `0` clears the selected override.
    /// - `flags`: `TPBLK_NEXT` for the next applicable call, or `TPBLK_ALL` for this context’s
    ///   calls.
    pub fn tpsblktime(&self, tout: i32, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsblktime(tout as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsblktime(self.c_ctx_ptr(), tout as c_int, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Return a selected context timeout, or the effective timeout, in seconds.
    ///
    /// # Arguments
    ///
    /// - `flags`: `TPBLK_NEXT` or `TPBLK_ALL` to query an override; `0` resolves the effective
    ///   timeout.
    pub fn tpgblktime(&self, flags: i64) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgblktime(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgblktime(self.c_ctx_ptr(), flags as c_long) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }
}
