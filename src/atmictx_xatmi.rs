#[cfg(endurox_epoll)]
use crate::AtmiError;
use crate::{raw, AtmiCtx, AtmiResult, TpContext, TpTranId, TypedBuffer, TypedUbf};
use core::ffi::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::ptr;
use std::time::Duration;
#[cfg(endurox_epoll)]
use std::time::Instant;

impl AtmiCtx {
    /// Synchronous RPC call with separate input and output buffers.
    ///
    /// This mirrors the C API: `idata` is the request buffer, and `odata` is the
    /// reply buffer. Enduro/X may reallocate `odata`; on success this wrapper is
    /// updated to the returned pointer.
    pub fn tpcall(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let mut reply = odata.as_ptr();
        self.tpcall_raw(svc, idata.as_ptr(), &mut reply, flags)?;
        odata.replace_ptr(reply);
        Ok(())
    }

    fn tpcall_raw(
        &self,
        svc: &str,
        idata: *mut c_char,
        odata: &mut *mut c_char,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpcall(
                c_svc.as_ptr() as *mut c_char,
                idata,
                0,
                odata,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpcall(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                idata,
                0,
                odata,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Asynchronous RPC call.  Returns a call descriptor used with `tpgetrply`.
    pub fn tpacall(&self, svc: &str, data: &TypedBuffer<'_>, flags: i64) -> AtmiResult<i32> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpacall(
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpacall(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                0,
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
    /// `cd` is updated by the framework when `TPGETANY` is used.
    pub fn tpgetrply(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let mut c_cd = *cd as c_int;
        let mut odata = data.as_ptr();
        let mut olen: c_long = 0;

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

        if rc == raw::EXSUCCEED as c_int {
            *cd = c_cd as i32;
            data.replace_ptr(odata);
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Cancel a pending asynchronous call descriptor returned by `tpacall`.
    pub fn tpcancel(&self, cd: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcancel(cd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcancel(self.c_ctx_ptr(), cd as c_int) };

        self.rc_to_result(rc)
    }

    /// Synchronous call that uses the async/reply-queue path only on pollable
    /// Enduro/X builds.
    ///
    /// On `EX_USE_EPOLL` builds this performs `tpacall`, waits for readiness on
    /// the internal reply queue descriptor, then drains the requested call
    /// descriptor with `tpgetrply(TPNOBLOCK)`. Other queue backends are not
    /// externally pollable, so this falls back to the normal blocking `tpcall`
    /// path (`Otpcall` when `ctx-send` is enabled).
    pub fn tpcall_polled(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
        timeout: Option<Duration>,
    ) -> AtmiResult<()> {
        #[cfg(not(endurox_epoll))]
        {
            let _ = timeout;
            return self.tpcall(svc, idata, odata, flags);
        }

        #[cfg(endurox_epoll)]
        {
            return self.tpcall_epoll(svc, idata, odata, flags, timeout);
        }
    }

    #[cfg(endurox_epoll)]
    fn tpcall_epoll(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
        timeout: Option<Duration>,
    ) -> AtmiResult<()> {
        let mut cd = self.tpacall(svc, idata, flags)?;
        self.tpgetrply_polled(&mut cd, odata, flags, timeout)
    }

    #[cfg(endurox_epoll)]
    fn tpgetrply_polled(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
        timeout: Option<Duration>,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let reply_fd = unsafe { raw::tpext_getreplyqfd() };

        #[cfg(feature = "ctx-send")]
        let reply_fd = unsafe { raw::Otpext_getreplyqfd(self.c_ctx_ptr()) };

        if reply_fd < 0 {
            return Err(self.atmi_last_error());
        }

        let deadline = timeout.map(|t| Instant::now() + t);
        let get_flags = flags | raw::TPNOBLOCK as i64;

        loop {
            if !self.poll_reply_queue(reply_fd, deadline)? {
                let _ = self.tpcancel(*cd);
                return Err(AtmiError::new(raw::TPETIME, "polled tpcall timed out"));
            }

            match self.tpgetrply(cd, data, get_flags) {
                Ok(()) => return Ok(()),
                Err(err) if err.code == raw::TPEBLOCK => continue,
                Err(err) => return Err(err),
            }
        }
    }

    #[cfg(endurox_epoll)]
    fn poll_reply_queue(&self, reply_fd: c_int, deadline: Option<Instant>) -> AtmiResult<bool> {
        let timeout_ms = match deadline {
            Some(d) => d
                .checked_duration_since(Instant::now())
                .map(|remaining| remaining.as_millis().min(c_int::MAX as u128) as c_int)
                .unwrap_or(0),
            None => -1,
        };

        let mut pfd = libc::pollfd {
            fd: reply_fd,
            events: libc::POLLIN,
            revents: 0,
        };

        loop {
            let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if rc > 0 {
                return Ok((pfd.revents & libc::POLLIN) != 0);
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

    pub fn tpconnect(
        &self,
        svc: &str,
        data: &TypedBuffer<'_>,
        len: usize,
        flags: i64,
    ) -> AtmiResult<i32> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpconnect(
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpconnect(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
            )
        };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    pub fn tpdiscon(&self, cd: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpdiscon(cd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpdiscon(self.c_ctx_ptr(), cd as c_int) };

        self.rc_to_result(rc)
    }

    pub fn tprecv(
        &self,
        cd: i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<(usize, i64)> {
        let mut odata = data.as_ptr();
        let mut olen: c_long = 0;
        let mut revent: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tprecv(
                cd as c_int,
                &mut odata,
                &mut olen,
                flags as c_long,
                &mut revent,
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
                &mut revent,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            data.replace_ptr(odata);
            Ok((olen as usize, revent as i64))
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpsend(
        &self,
        cd: i32,
        data: &TypedBuffer<'_>,
        len: usize,
        flags: i64,
    ) -> AtmiResult<i64> {
        let mut revent: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpsend(
                cd as c_int,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
                &mut revent,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpsend(
                self.c_ctx_ptr(),
                cd as c_int,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
                &mut revent,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(revent as i64)
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpabort(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpabort(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpabort(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpscmt(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpscmt(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpscmt(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpbegin(&self, timeout: u64, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpbegin(timeout as _, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpbegin(self.c_ctx_ptr(), timeout as _, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpcommit(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcommit(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcommit(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Suspend the current global transaction. Returns a `TpTranId` that can
    /// later be passed to `tpresume` to rejoin the transaction.
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
    pub fn tpresume(&self, tranid: &TpTranId, flags: i64) -> AtmiResult<()> {
        let mut inner = tranid.0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpresume(&mut inner, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpresume(self.c_ctx_ptr(), &mut inner, flags as c_long) };

        self.rc_to_result(rc)
    }

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

    pub fn tpsubscribe(
        &self,
        eventexpr: &str,
        filter: Option<&str>,
        ctl: Option<&mut crate::TpEvCtl>,
        flags: i64,
    ) -> AtmiResult<i64> {
        let c_expr = CString::new(eventexpr).map_err(|_| self.atmi_last_error())?;
        let c_filter = filter
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;
        let filter_ptr = c_filter
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());
        let ctl_ptr = ctl.map_or(ptr::null_mut(), |ctl| ctl.as_mut_ptr());

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

    pub fn tpunsubscribe(&self, subscription: i64, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpunsubscribe(subscription as c_long, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpunsubscribe(self.c_ctx_ptr(), subscription as c_long, flags as c_long)
        };

        self.rc_to_result(rc)
    }

    pub fn tppost(&self, eventname: &str, data: &TypedBuffer<'_>, flags: i64) -> AtmiResult<()> {
        let c_event = CString::new(eventname).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tppost(
                c_event.as_ptr() as *mut c_char,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otppost(
                self.c_ctx_ptr(),
                c_event.as_ptr() as *mut c_char,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Initialize an application thread with no authentication (null TPINIT).
    pub fn tpappthrinit(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpappthrinit(ptr::null_mut()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpappthrinit(self.c_ctx_ptr(), ptr::null_mut()) };

        self.rc_to_result(rc)
    }

    pub fn tpappthrterm(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpappthrterm() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpappthrterm(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    pub fn tpchkauth(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpchkauth() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpchkauth(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Send an unsolicited message to a specific client.
    pub fn tpnotify(
        &self,
        clientid: &mut crate::ClientId,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpnotify(clientid, data.as_ptr(), 0, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpnotify(
                self.c_ctx_ptr(),
                clientid,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Broadcast an unsolicited message to matching clients.
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
        let rc =
            unsafe { raw::tpbroadcast(p_lmid, p_usr, p_clt, data.as_ptr(), 0, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpbroadcast(
                self.c_ctx_ptr(),
                p_lmid,
                p_usr,
                p_clt,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    pub fn tpchkunsol(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpchkunsol() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpchkunsol(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    pub fn tptoutset(&self, tout: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptoutset(tout as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptoutset(self.c_ctx_ptr(), tout as c_int) };

        self.rc_to_result(rc)
    }

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
            let _ = olen;
            Ok(unsafe { TypedBuffer::from_raw(self, obuf) })
        } else {
            Err(self.atmi_last_error())
        }
    }

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
                0,
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
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Dequeue a buffer from a persistent queue. Returns the dequeued buffer.
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
            Ok(unsafe { TypedBuffer::from_raw(self, odata) })
        } else {
            Err(self.atmi_last_error())
        }
    }

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
                0,
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
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Dequeue a buffer by node/server ID. Returns the dequeued buffer.
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
            Ok(unsafe { TypedBuffer::from_raw(self, odata) })
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Capture the current ATMI context handle.
    pub fn tpgetctxt(&self) -> AtmiResult<TpContext> {
        let mut out: raw::TPCONTEXT_T = ptr::null_mut();

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetctxt(&mut out, 0) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetctxt(self.c_ctx_ptr(), &mut out, 0) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(TpContext(out))
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Activate a previously captured ATMI context handle on the current thread.
    pub fn tpsetctxt(&self, context: TpContext, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsetctxt(context.0, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsetctxt(self.c_ctx_ptr(), context.0, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpencrypt(&self, input: &[u8], flags: i64) -> AtmiResult<Vec<u8>> {
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

    pub fn tpdecrypt(&self, input: &[u8], flags: i64) -> AtmiResult<Vec<u8>> {
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

    pub fn tpsprio(&self, prio: i32, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsprio(prio as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsprio(self.c_ctx_ptr(), prio as c_int, flags as c_long) };

        self.rc_to_result(rc)
    }

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

    pub fn tpsblktime(&self, tout: i32, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsblktime(tout as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsblktime(self.c_ctx_ptr(), tout as c_int, flags as c_long) };

        self.rc_to_result(rc)
    }

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
