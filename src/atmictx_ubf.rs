use crate::raw::*;
use crate::{raw, AtmiCtx, TypedUbf, UbfResult};

/// UBF field type for safe field-id construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UbfFieldType {
    /// `BFLD_SHORT`.
    Short,
    /// `BFLD_LONG`.
    Long,
    /// `BFLD_CHAR`.
    Char,
    /// `BFLD_FLOAT`.
    Float,
    /// `BFLD_DOUBLE`.
    Double,
    /// `BFLD_STRING`.
    String,
    /// `BFLD_CARRAY`.
    Carray,
    /// `BFLD_PTR`.
    Ptr,
    /// `BFLD_UBF`.
    Ubf,
    /// `BFLD_VIEW`.
    View,
}

impl UbfFieldType {
    #[inline]
    fn as_raw(self) -> i32 {
        match self {
            UbfFieldType::Short => raw::BFLD_SHORT as i32,
            UbfFieldType::Long => raw::BFLD_LONG as i32,
            UbfFieldType::Char => raw::BFLD_CHAR as i32,
            UbfFieldType::Float => raw::BFLD_FLOAT as i32,
            UbfFieldType::Double => raw::BFLD_DOUBLE as i32,
            UbfFieldType::String => raw::BFLD_STRING as i32,
            UbfFieldType::Carray => raw::BFLD_CARRAY as i32,
            UbfFieldType::Ptr => raw::BFLD_PTR as i32,
            UbfFieldType::Ubf => raw::BFLD_UBF as i32,
            UbfFieldType::View => raw::BFLD_VIEW as i32,
        }
    }
}

impl AtmiCtx {
    /// Full low-level UBF API wrappers routed through this context.
    ///
    /// In `ctx-send` mode these call `O*`/`OCB*`/`Ondrx_*` functions with
    /// associated context; otherwise they call plain `B*`/`CB*` variants.

    #[inline]
    fn ubf_unit_result(&self, rc: ::std::os::raw::c_int) -> UbfResult<()> {
        if rc == raw::EXSUCCEED as ::std::os::raw::c_int {
            Ok(())
        } else {
            Err(self.ubf_last_error())
        }
    }

    #[inline]
    fn ubf_count_result<T>(&self, rc: T) -> UbfResult<usize>
    where
        T: Into<i64> + Copy,
    {
        let value = rc.into();
        if value < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(value as usize)
        }
    }

    #[inline]
    pub(crate) unsafe fn b16to32(&self, dest: *mut UBFH, src: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::B16to32(dest, src)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OB16to32(self.c_ctx_ptr(), dest, src)
        }
    }

    #[inline]
    pub(crate) unsafe fn b32to16(&self, dest: *mut UBFH, src: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::B32to16(dest, src)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OB32to16(self.c_ctx_ptr(), dest, src)
        }
    }

    #[inline]
    pub(crate) unsafe fn b_error(&self, str_: *mut ::std::os::raw::c_char) {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::B_error(str_)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OB_error(self.c_ctx_ptr(), str_)
        }
    }

    #[inline]
    pub(crate) unsafe fn badd(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Badd(p_ub, bfldid, buf, len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBadd(self.c_ctx_ptr(), p_ub, bfldid, buf, len)
        }
    }

    #[inline]
    pub(crate) unsafe fn baddfast(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
        next_fld: *mut Bfld_loc_info_t,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Baddfast(p_ub, bfldid, buf, len, next_fld)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBaddfast(self.c_ctx_ptr(), p_ub, bfldid, buf, len, next_fld)
        }
    }

    #[inline]
    pub(crate) unsafe fn badds(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        buf: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Badds(p_ub, bfldid, buf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBadds(self.c_ctx_ptr(), p_ub, bfldid, buf)
        }
    }

    #[inline]
    pub(crate) unsafe fn balloc(&self, f: BFLDOCC, v: BFLDLEN) -> *mut UBFH {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Balloc(f, v)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBalloc(self.c_ctx_ptr(), f, v)
        }
    }

    #[inline]
    pub(crate) unsafe fn bboolco(
        &self,
        expr: *mut ::std::os::raw::c_char,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bboolco(expr)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBboolco(self.c_ctx_ptr(), expr)
        }
    }

    #[inline]
    pub(crate) unsafe fn bboolev(
        &self,
        p_ub: *mut UBFH,
        tree: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bboolev(p_ub, tree)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBboolev(self.c_ctx_ptr(), p_ub, tree)
        }
    }

    #[inline]
    pub(crate) unsafe fn bboolpr(&self, tree: *mut ::std::os::raw::c_char, outf: *mut FILE) {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bboolpr(tree, outf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBboolpr(self.c_ctx_ptr(), tree, outf)
        }
    }

    #[inline]
    pub(crate) unsafe fn bboolprcb(
        &self,
        tree: *mut ::std::os::raw::c_char,
        p_writef: ::std::option::Option<
            unsafe extern "C" fn(
                buffer: *mut ::std::os::raw::c_char,
                datalen: ::std::os::raw::c_long,
                dataptr1: *mut ::std::os::raw::c_void,
            ) -> ::std::os::raw::c_int,
        >,
        dataptr1: *mut ::std::os::raw::c_void,
    ) {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bboolprcb(tree, p_writef, dataptr1)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBboolprcb(self.c_ctx_ptr(), tree, p_writef, dataptr1)
        }
    }

    #[inline]
    pub(crate) unsafe fn bboolsetcbf(
        &self,
        funcname: *mut ::std::os::raw::c_char,
        functionPtr: ::std::option::Option<
            unsafe extern "C" fn(
                p_ub: *mut UBFH,
                funcname: *mut ::std::os::raw::c_char,
            ) -> ::std::os::raw::c_long,
        >,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bboolsetcbf(funcname, functionPtr)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBboolsetcbf(self.c_ctx_ptr(), funcname, functionPtr)
        }
    }

    #[inline]
    pub(crate) unsafe fn bboolsetcbf2(
        &self,
        funcname: *mut ::std::os::raw::c_char,
        functionPtr: ::std::option::Option<
            unsafe extern "C" fn(
                p_ub: *mut UBFH,
                funcname: *mut ::std::os::raw::c_char,
                arg1: *mut ::std::os::raw::c_char,
            ) -> ::std::os::raw::c_long,
        >,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bboolsetcbf2(funcname, functionPtr)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBboolsetcbf2(self.c_ctx_ptr(), funcname, functionPtr)
        }
    }

    #[inline]
    pub(crate) unsafe fn bchg(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bchg(p_ub, bfldid, occ, buf, len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBchg(self.c_ctx_ptr(), p_ub, bfldid, occ, buf, len)
        }
    }

    #[inline]
    pub(crate) unsafe fn bchgs(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bchgs(p_ub, bfldid, occ, buf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBchgs(self.c_ctx_ptr(), p_ub, bfldid, occ, buf)
        }
    }

    #[inline]
    /// Return whether two UBF buffers contain the same fields and values.
    ///
    /// Uses `Bcmp(3)` internally.
    pub fn bcmp(&self, ubf1: &TypedUbf<'_>, ubf2: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        {
            unsafe { raw::Bcmp(ubf1.as_ubfh(), ubf2.as_ubfh()) == 0 }
        }

        #[cfg(feature = "ctx-send")]
        {
            unsafe { raw::OBcmp(self.c_ctx_ptr(), ubf1.as_ubfh(), ubf2.as_ubfh()) == 0 }
        }
    }

    #[inline]
    /// Append all fields from `src` into `dst`.
    ///
    /// Wraps `Bconcat(3)`. Existing destination occurrences are kept and source
    /// occurrences are appended.
    pub fn bconcat(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bconcat(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBconcat(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Copy the full contents of `src` into `dst`.
    ///
    /// Wraps `Bcpy(3)`. The destination buffer must be large enough for the
    /// copied data.
    pub fn bcpy(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bcpy(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBcpy(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Delete one occurrence of a field from a UBF buffer.
    ///
    /// Wraps `Bdel(3)`. Occurrences after the deleted one shift down.
    pub fn bdel(&self, ubf: &mut TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdel(ubf.as_ubfh(), bfldid, occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdel(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Delete all occurrences of a field from a UBF buffer.
    ///
    /// Wraps `Bdelall(3)`.
    pub fn bdelall(&self, ubf: &mut TypedUbf<'_>, bfldid: BFLDID) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdelall(ubf.as_ubfh(), bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdelall(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Delete all fields listed in `fldlist` from a UBF buffer.
    ///
    /// Wraps `Bdelete(3)`. The field list must be terminated with `0`, matching
    /// the Enduro/X C API convention.
    pub fn bdelete(&self, ubf: &mut TypedUbf<'_>, fldlist: &mut [i32]) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdelete(ubf.as_ubfh(), fldlist.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdelete(self.c_ctx_ptr(), ubf.as_ubfh(), fldlist.as_mut_ptr()) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    pub(crate) unsafe fn becodestr(
        &self,
        err: ::std::os::raw::c_int,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Becodestr(err)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBecodestr(self.c_ctx_ptr(), err)
        }
    }

    #[inline]
    pub(crate) unsafe fn bextread(&self, p_ub: *mut UBFH, inf: *mut FILE) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bextread(p_ub, inf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBextread(self.c_ctx_ptr(), p_ub, inf)
        }
    }

    #[inline]
    pub(crate) unsafe fn bextreadcb(
        &self,
        p_ub: *mut UBFH,
        p_readf: ::std::option::Option<
            unsafe extern "C" fn(
                buffer: *mut ::std::os::raw::c_char,
                bufsz: ::std::os::raw::c_long,
                dataptr1: *mut ::std::os::raw::c_void,
            ) -> ::std::os::raw::c_long,
        >,
        dataptr1: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bextreadcb(p_ub, p_readf, dataptr1)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBextreadcb(self.c_ctx_ptr(), p_ub, p_readf, dataptr1)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfind(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        p_len: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfind(p_ub, bfldid, occ, p_len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfind(self.c_ctx_ptr(), p_ub, bfldid, occ, p_len)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfindlast(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: *mut BFLDOCC,
        len: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfindlast(p_ub, bfldid, occ, len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfindlast(self.c_ctx_ptr(), p_ub, bfldid, occ, len)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfindocc(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
    ) -> BFLDOCC {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfindocc(p_ub, bfldid, buf, len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfindocc(self.c_ctx_ptr(), p_ub, bfldid, buf, len)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfindr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        p_len: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfindr(p_ub, fldidocc, p_len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfindr(self.c_ctx_ptr(), p_ub, fldidocc, p_len)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfinds(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfinds(p_ub, bfldid, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfinds(self.c_ctx_ptr(), p_ub, bfldid, occ)
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbadd(
        &self,
        txn: *mut EDB_txn,
        fldtype: ::std::os::raw::c_short,
        bfldno: BFLDID,
        fldname: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbadd(txn, fldtype, bfldno, fldname)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbadd(self.c_ctx_ptr(), txn, fldtype, bfldno, fldname)
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbdel(
        &self,
        txn: *mut EDB_txn,
        bfldid: BFLDID,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbdel(txn, bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbdel(self.c_ctx_ptr(), txn, bfldid)
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbdrop(&self, txn: *mut EDB_txn) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbdrop(txn)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbdrop(self.c_ctx_ptr(), txn)
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbget(
        &self,
        data: *mut EDB_val,
        p_fldtype: *mut ::std::os::raw::c_short,
        p_bfldno: *mut BFLDID,
        p_bfldid: *mut BFLDID,
        fldname: *mut ::std::os::raw::c_char,
        fldname_bufsz: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbget(data, p_fldtype, p_bfldno, p_bfldid, fldname, fldname_bufsz)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbget(
                self.c_ctx_ptr(),
                data,
                p_fldtype,
                p_bfldno,
                p_bfldid,
                fldname,
                fldname_bufsz,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbid(&self, fldname: *mut ::std::os::raw::c_char) -> BFLDID {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbid(fldname)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbid(self.c_ctx_ptr(), fldname)
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbload(&self) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbload()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbload(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbname(&self, bfldid: BFLDID) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbname(bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbname(self.c_ctx_ptr(), bfldid)
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbunlink(&self) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbunlink()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbunlink(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) unsafe fn bflddbunload(&self) {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bflddbunload()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBflddbunload(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) unsafe fn bfldddbgetenv(
        &self,
        dbi_id: *mut *mut EDB_dbi,
        dbi_nm: *mut *mut EDB_dbi,
    ) -> *mut EDB_env {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfldddbgetenv(dbi_id, dbi_nm)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfldddbgetenv(self.c_ctx_ptr(), dbi_id, dbi_nm)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfldid(&self, fldnm: *mut ::std::os::raw::c_char) -> BFLDID {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfldid(fldnm)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfldid(self.c_ctx_ptr(), fldnm)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfldno(&self, bfldid: BFLDID) -> BFLDOCC {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfldno(bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfldno(self.c_ctx_ptr(), bfldid)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfldtype(&self, bfldid: BFLDID) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfldtype(bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfldtype(self.c_ctx_ptr(), bfldid)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfloatev(
        &self,
        p_ub: *mut UBFH,
        tree: *mut ::std::os::raw::c_char,
    ) -> f64 {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfloatev(p_ub, tree)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfloatev(self.c_ctx_ptr(), p_ub, tree)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfname(&self, bfldid: BFLDID) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfname(bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfname(self.c_ctx_ptr(), bfldid)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfprint(&self, p_ub: *mut UBFH, outf: *mut FILE) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfprint(p_ub, outf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfprint(self.c_ctx_ptr(), p_ub, outf)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfprintcb(
        &self,
        p_ub: *mut UBFH,
        p_writef: ndrx_plugin_tplogprintubf_hook_t,
        dataptr1: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfprintcb(p_ub, p_writef, dataptr1)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfprintcb(self.c_ctx_ptr(), p_ub, p_writef, dataptr1)
        }
    }

    #[inline]
    pub(crate) unsafe fn bfree(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bfree(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBfree(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub(crate) unsafe fn bget(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        buflen: *mut BFLDLEN,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bget(p_ub, bfldid, occ, buf, buflen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBget(self.c_ctx_ptr(), p_ub, bfldid, occ, buf, buflen)
        }
    }

    #[inline]
    pub(crate) unsafe fn bgetalloc(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        extralen: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bgetalloc(p_ub, bfldid, occ, extralen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBgetalloc(self.c_ctx_ptr(), p_ub, bfldid, occ, extralen)
        }
    }

    #[inline]
    pub(crate) unsafe fn bgetlast(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: *mut BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: *mut BFLDLEN,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bgetlast(p_ub, bfldid, occ, buf, len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBgetlast(self.c_ctx_ptr(), p_ub, bfldid, occ, buf, len)
        }
    }

    #[inline]
    pub(crate) unsafe fn bgetr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        buf: *mut ::std::os::raw::c_char,
        buflen: *mut BFLDLEN,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bgetr(p_ub, fldidocc, buf, buflen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBgetr(self.c_ctx_ptr(), p_ub, fldidocc, buf, buflen)
        }
    }

    #[inline]
    pub(crate) unsafe fn bgets(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bgets(p_ub, bfldid, occ, buf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBgets(self.c_ctx_ptr(), p_ub, bfldid, occ, buf)
        }
    }

    #[inline]
    pub(crate) unsafe fn bgetsa(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        extralen: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bgetsa(p_ub, bfldid, occ, extralen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBgetsa(self.c_ctx_ptr(), p_ub, bfldid, occ, extralen)
        }
    }

    #[inline]
    /// Return the number of index slots used by a UBF buffer.
    ///
    /// Wraps `Bidxused(3)`.
    pub fn bidxused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bidxused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBidxused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    #[inline]
    /// Build or rebuild the UBF index.
    ///
    /// Wraps `Bindex(3)`. The `occ` argument is passed directly to Enduro/X.
    pub fn bindex(&self, ubf: &mut TypedUbf<'_>, occ: BFLDOCC) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bindex(ubf.as_ubfh(), occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBindex(self.c_ctx_ptr(), ubf.as_ubfh(), occ) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    pub(crate) unsafe fn binit(&self, p_ub: *mut UBFH, len: BFLDLEN) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Binit(p_ub, len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBinit(self.c_ctx_ptr(), p_ub, len)
        }
    }

    #[inline]
    /// Return whether a buffer is a valid UBF buffer.
    ///
    /// Uses `Bisubf(3)` internally.
    pub fn bisubf(&self, ubf: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        {
            unsafe { raw::Bisubf(ubf.as_ubfh()) != 0 }
        }

        #[cfg(feature = "ctx-send")]
        {
            unsafe { raw::OBisubf(self.c_ctx_ptr(), ubf.as_ubfh()) != 0 }
        }
    }

    #[inline]
    /// Join fields from `src` into `dest`.
    ///
    /// Wraps `Bjoin(3)`. Enduro/X applies the merge semantics defined by the C
    /// API.
    pub fn bjoin(&self, dest: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bjoin(dest.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBjoin(self.c_ctx_ptr(), dest.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Return the stored length of a field occurrence.
    ///
    /// Wraps `Blen(3)`.
    pub fn blen(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Blen(ubf.as_ubfh(), bfldid, occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBlen(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) };

        self.ubf_count_result(rc)
    }

    #[inline]
    pub(crate) unsafe fn bmkfldid(&self, fldtype: ::std::os::raw::c_int, bfldid: BFLDID) -> BFLDID {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bmkfldid(fldtype, bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBmkfldid(self.c_ctx_ptr(), fldtype, bfldid)
        }
    }

    #[inline]
    pub(crate) unsafe fn bneeded(
        &self,
        nrfields: BFLDOCC,
        totsize: BFLDLEN,
    ) -> ::std::os::raw::c_long {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bneeded(nrfields, totsize)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBneeded(self.c_ctx_ptr(), nrfields, totsize)
        }
    }

    #[inline]
    pub(crate) unsafe fn bnext(
        &self,
        p_ub: *mut UBFH,
        bfldid: *mut BFLDID,
        occ: *mut BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: *mut BFLDLEN,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bnext(p_ub, bfldid, occ, buf, len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBnext(self.c_ctx_ptr(), p_ub, bfldid, occ, buf, len)
        }
    }

    #[inline]
    pub(crate) unsafe fn bnext2(
        &self,
        bnext_state: *mut Bnext_state_t,
        p_ub: *mut UBFH,
        bfldid: *mut BFLDID,
        occ: *mut BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: *mut BFLDLEN,
        d_ptr: *mut *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bnext2(bnext_state, p_ub, bfldid, occ, buf, len, d_ptr)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBnext2(
                self.c_ctx_ptr(),
                bnext_state,
                p_ub,
                bfldid,
                occ,
                buf,
                len,
                d_ptr,
            )
        }
    }

    #[inline]
    /// Return the total number of field occurrences in a UBF buffer.
    ///
    /// Wraps `Bnum(3)`.
    pub fn bnum(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bnum(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBnum(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    #[inline]
    /// Return the number of occurrences for one field.
    ///
    /// Wraps `Boccur(3)`.
    pub fn boccur(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Boccur(ubf.as_ubfh(), bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBoccur(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid) };

        self.ubf_count_result(rc)
    }

    #[inline]
    /// Outer-join fields from `src` into `dest`.
    ///
    /// Wraps `Bojoin(3)`. Enduro/X applies the outer-join merge semantics
    /// defined by the C API.
    pub fn bojoin(&self, dest: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bojoin(dest.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBojoin(self.c_ctx_ptr(), dest.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Return whether a field occurrence is present.
    ///
    /// Uses `Bpres(3)` internally.
    pub fn bpres(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        {
            unsafe { raw::Bpres(ubf.as_ubfh(), bfldid, occ) != 0 }
        }

        #[cfg(feature = "ctx-send")]
        {
            unsafe { raw::OBpres(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) != 0 }
        }
    }

    #[inline]
    pub(crate) unsafe fn bpresr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bpresr(p_ub, fldidocc)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBpresr(self.c_ctx_ptr(), p_ub, fldidocc)
        }
    }

    #[inline]
    pub(crate) unsafe fn bprint(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bprint(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBprint(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    /// Project a UBF buffer in place to the fields listed in `fldlist`.
    ///
    /// Wraps `Bproj(3)`. The field list must be terminated with `0`, matching
    /// the Enduro/X C API convention.
    pub fn bproj(&self, ubf: &mut TypedUbf<'_>, fldlist: &mut [i32]) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bproj(ubf.as_ubfh(), fldlist.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBproj(self.c_ctx_ptr(), ubf.as_ubfh(), fldlist.as_mut_ptr()) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Copy a projection of `src` into `dst`.
    ///
    /// Wraps `Bprojcpy(3)`. The field list must be terminated with `0`.
    pub fn bprojcpy(
        &self,
        dst: &mut TypedUbf<'_>,
        src: &TypedUbf<'_>,
        fldlist: &mut [i32],
    ) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bprojcpy(dst.as_ubfh(), src.as_ubfh(), fldlist.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBprojcpy(
                self.c_ctx_ptr(),
                dst.as_ubfh(),
                src.as_ubfh(),
                fldlist.as_mut_ptr(),
            )
        };

        self.ubf_unit_result(rc)
    }

    #[inline]
    pub(crate) unsafe fn bread(&self, p_ub: *mut UBFH, inf: *mut FILE) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bread(p_ub, inf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBread(self.c_ctx_ptr(), p_ub, inf)
        }
    }

    /// `oatmi.h` does not expose an `OBreadcb` variant; this method falls back
    /// to the global API in `ctx-send` mode.
    #[inline]
    pub(crate) unsafe fn breadcb(
        &self,
        p_ub: *mut UBFH,
        p_readf: ::std::option::Option<
            unsafe extern "C" fn(
                buffer: *mut ::std::os::raw::c_char,
                bufsz: ::std::os::raw::c_long,
                dataptr1: *mut ::std::os::raw::c_void,
            ) -> ::std::os::raw::c_long,
        >,
        dataptr1: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Breadcb(p_ub, p_readf, dataptr1)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Breadcb(p_ub, p_readf, dataptr1)
        }
    }

    #[inline]
    pub(crate) unsafe fn brealloc(&self, p_ub: *mut UBFH, f: BFLDOCC, v: BFLDLEN) -> *mut UBFH {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Brealloc(p_ub, f, v)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBrealloc(self.c_ctx_ptr(), p_ub, f, v)
        }
    }

    #[inline]
    pub(crate) unsafe fn brstrindex(&self, p_ub: *mut UBFH, occ: BFLDOCC) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Brstrindex(p_ub, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBrstrindex(self.c_ctx_ptr(), p_ub, occ)
        }
    }

    #[inline]
    /// Return the allocated size of a UBF buffer in bytes.
    ///
    /// Wraps `Bsizeof(3)`.
    pub fn bsizeof(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bsizeof(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBsizeof(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    #[inline]
    pub(crate) unsafe fn bstrerror(
        &self,
        err: ::std::os::raw::c_int,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bstrerror(err)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBstrerror(self.c_ctx_ptr(), err)
        }
    }

    #[inline]
    /// Return whether `ubf1` is a subset of `ubf2`.
    ///
    /// Uses `Bsubset(3)` internally.
    pub fn bsubset(&self, ubf1: &TypedUbf<'_>, ubf2: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        {
            unsafe { raw::Bsubset(ubf1.as_ubfh(), ubf2.as_ubfh()) != 0 }
        }

        #[cfg(feature = "ctx-send")]
        {
            unsafe { raw::OBsubset(self.c_ctx_ptr(), ubf1.as_ubfh(), ubf2.as_ubfh()) != 0 }
        }
    }

    #[inline]
    pub(crate) unsafe fn btreefree(&self, tree: *mut ::std::os::raw::c_char) {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Btreefree(tree)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBtreefree(self.c_ctx_ptr(), tree)
        }
    }

    #[inline]
    pub(crate) unsafe fn btypcvt(
        &self,
        to_len: *mut BFLDLEN,
        to_type: ::std::os::raw::c_int,
        from_buf: *mut ::std::os::raw::c_char,
        from_type: ::std::os::raw::c_int,
        from_len: BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Btypcvt(to_len, to_type, from_buf, from_type, from_len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBtypcvt(
                self.c_ctx_ptr(),
                to_len,
                to_type,
                from_buf,
                from_type,
                from_len,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn btype(&self, bfldid: BFLDID) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Btype(bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBtype(self.c_ctx_ptr(), bfldid)
        }
    }

    #[inline]
    /// Remove the index from a UBF buffer.
    ///
    /// Wraps `Bunindex(3)`.
    pub fn bunindex(&self, ubf: &mut TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bunindex(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBunindex(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    #[inline]
    /// Return the unused byte count in a UBF buffer.
    ///
    /// Wraps `Bunused(3)`.
    pub fn bunused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bunused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBunused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    #[inline]
    /// Update `dst` with fields from `src`.
    ///
    /// Wraps `Bupdate(3)`. Source fields replace matching destination fields
    /// according to Enduro/X UBF update semantics.
    pub fn bupdate(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bupdate(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBupdate(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    #[inline]
    /// Return the used byte count in a UBF buffer.
    ///
    /// Wraps `Bused(3)`.
    pub fn bused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    #[inline]
    pub(crate) unsafe fn bvcmp(
        &self,
        cstruct1: *mut ::std::os::raw::c_char,
        view1: *mut ::std::os::raw::c_char,
        cstruct2: *mut ::std::os::raw::c_char,
        view2: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvcmp(cstruct1, view1, cstruct2, view2)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvcmp(self.c_ctx_ptr(), cstruct1, view1, cstruct2, view2)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvcpy(
        &self,
        cstruct_dst: *mut ::std::os::raw::c_char,
        cstruct_src: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_long {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvcpy(cstruct_dst, cstruct_src, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvcpy(self.c_ctx_ptr(), cstruct_dst, cstruct_src, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvextread(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        inf: *mut FILE,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvextread(cstruct, view, inf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvextread(self.c_ctx_ptr(), cstruct, view, inf)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvextreadcb(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        p_readf: ::std::option::Option<
            unsafe extern "C" fn(
                buffer: *mut ::std::os::raw::c_char,
                bufsz: ::std::os::raw::c_long,
                dataptr1: *mut ::std::os::raw::c_void,
            ) -> ::std::os::raw::c_long,
        >,
        dataptr1: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvextreadcb(cstruct, view, p_readf, dataptr1)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvextreadcb(self.c_ctx_ptr(), cstruct, view, p_readf, dataptr1)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvfprint(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        outf: *mut FILE,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvfprint(cstruct, view, outf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvfprint(self.c_ctx_ptr(), cstruct, view, outf)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvfprintcb(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        p_writef: ndrx_plugin_tplogprintubf_hook_t,
        dataptr1: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvfprintcb(cstruct, view, p_writef, dataptr1)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvfprintcb(self.c_ctx_ptr(), cstruct, view, p_writef, dataptr1)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvftos(
        &self,
        p_ub: *mut UBFH,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvftos(p_ub, cstruct, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvftos(self.c_ctx_ptr(), p_ub, cstruct, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvnext(
        &self,
        state: *mut Bvnext_state_t,
        view: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        fldtype: *mut ::std::os::raw::c_int,
        maxocc: *mut BFLDOCC,
        dim_size: *mut ::std::os::raw::c_long,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvnext(state, view, cname, fldtype, maxocc, dim_size)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvnext(
                self.c_ctx_ptr(),
                state,
                view,
                cname,
                fldtype,
                maxocc,
                dim_size,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn bvnull(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvnull(cstruct, cname, occ, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvnull(self.c_ctx_ptr(), cstruct, cname, occ, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvnullr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvnullr(p_ub, fldidocc, cname, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvnullr(self.c_ctx_ptr(), p_ub, fldidocc, cname, occ)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvoccur(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        maxocc: *mut BFLDOCC,
        realocc: *mut BFLDOCC,
        dim_size: *mut ::std::os::raw::c_long,
        fldtype: *mut ::std::os::raw::c_int,
    ) -> BFLDOCC {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvoccur(cstruct, view, cname, maxocc, realocc, dim_size, fldtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvoccur(
                self.c_ctx_ptr(),
                cstruct,
                view,
                cname,
                maxocc,
                realocc,
                dim_size,
                fldtype,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn bvopt(
        &self,
        cname: *mut ::std::os::raw::c_char,
        option: ::std::os::raw::c_int,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvopt(cname, option, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvopt(self.c_ctx_ptr(), cname, option, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvprint(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvprint(cstruct, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvprint(self.c_ctx_ptr(), cstruct, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvrefresh(&self) {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvrefresh()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvrefresh(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) unsafe fn bvselinit(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvselinit(cstruct, cname, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvselinit(self.c_ctx_ptr(), cstruct, cname, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvsetoccur(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvsetoccur(cstruct, view, cname, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvsetoccur(self.c_ctx_ptr(), cstruct, view, cname, occ)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvsinit(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvsinit(cstruct, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvsinit(self.c_ctx_ptr(), cstruct, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvsizeof(
        &self,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_long {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvsizeof(view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvsizeof(self.c_ctx_ptr(), view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bvstof(
        &self,
        p_ub: *mut UBFH,
        cstruct: *mut ::std::os::raw::c_char,
        mode: ::std::os::raw::c_int,
        view: *mut ::std::os::raw::c_char,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bvstof(p_ub, cstruct, mode, view)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBvstof(self.c_ctx_ptr(), p_ub, cstruct, mode, view)
        }
    }

    #[inline]
    pub(crate) unsafe fn bwrite(&self, p_ub: *mut UBFH, outf: *mut FILE) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bwrite(p_ub, outf)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBwrite(self.c_ctx_ptr(), p_ub, outf)
        }
    }

    #[inline]
    pub(crate) unsafe fn bwritecb(
        &self,
        p_ub: *mut UBFH,
        p_writef: ::std::option::Option<
            unsafe extern "C" fn(
                buffer: *mut ::std::os::raw::c_char,
                bufsz: ::std::os::raw::c_long,
                dataptr1: *mut ::std::os::raw::c_void,
            ) -> ::std::os::raw::c_long,
        >,
        dataptr1: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bwritecb(p_ub, p_writef, dataptr1)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBwritecb(self.c_ctx_ptr(), p_ub, p_writef, dataptr1)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbadd(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBadd(p_ub, bfldid, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBadd(self.c_ctx_ptr(), p_ub, bfldid, buf, len, usrtype)
        }
    }

    /// `oatmi.h` does not expose `OCBaddfast`; in `ctx-send` mode this falls
    /// back to `cbadd` (conversion retained, location hint ignored).
    #[inline]
    pub(crate) unsafe fn cbaddfast(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
        usrtype: ::std::os::raw::c_int,
        next_fld: *mut Bfld_loc_info_t,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBaddfast(p_ub, bfldid, buf, len, usrtype, next_fld)
        }

        #[cfg(feature = "ctx-send")]
        {
            let _ = next_fld;
            self.cbadd(p_ub, bfldid, buf, len, usrtype)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbchg(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBchg(p_ub, bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBchg(self.c_ctx_ptr(), p_ub, bfldid, occ, buf, len, usrtype)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbfind(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        len: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBfind(p_ub, bfldid, occ, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBfind(self.c_ctx_ptr(), p_ub, bfldid, occ, len, usrtype)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbfindocc(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> BFLDOCC {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBfindocc(p_ub, bfldid, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBfindocc(self.c_ctx_ptr(), p_ub, bfldid, buf, len, usrtype)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbfindr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        len: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBfindr(p_ub, fldidocc, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBfindr(self.c_ctx_ptr(), p_ub, fldidocc, len, usrtype)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbget(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBget(p_ub, bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBget(self.c_ctx_ptr(), p_ub, bfldid, occ, buf, len, usrtype)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbgetalloc(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
        usrtype: ::std::os::raw::c_int,
        extralen: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBgetalloc(p_ub, bfldid, occ, usrtype, extralen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBgetalloc(self.c_ctx_ptr(), p_ub, bfldid, occ, usrtype, extralen)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbgetallocr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        usrtype: ::std::os::raw::c_int,
        extralen: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBgetallocr(p_ub, fldidocc, usrtype, extralen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBgetallocr(self.c_ctx_ptr(), p_ub, fldidocc, usrtype, extralen)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbgetr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        buf: *mut ::std::os::raw::c_char,
        buflen: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBgetr(p_ub, fldidocc, buf, buflen, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBgetr(self.c_ctx_ptr(), p_ub, fldidocc, buf, buflen, usrtype)
        }
    }

    #[inline]
    pub(crate) unsafe fn cbvchg(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: BFLDLEN,
        usrtype: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBvchg(cstruct, view, cname, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBvchg(
                self.c_ctx_ptr(),
                cstruct,
                view,
                cname,
                occ,
                buf,
                len,
                usrtype,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn cbvget(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_long,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBvget(cstruct, view, cname, occ, buf, len, usrtype, flags)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBvget(
                self.c_ctx_ptr(),
                cstruct,
                view,
                cname,
                occ,
                buf,
                len,
                usrtype,
                flags,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn cbvgetalloc(
        &self,
        cstruct: *mut ::std::os::raw::c_char,
        view: *mut ::std::os::raw::c_char,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        usrtype: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_long,
        extralen: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBvgetalloc(cstruct, view, cname, occ, usrtype, flags, extralen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBvgetalloc(
                self.c_ctx_ptr(),
                cstruct,
                view,
                cname,
                occ,
                usrtype,
                flags,
                extralen,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn cbvgetallocr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        usrtype: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_long,
        extralen: *mut BFLDLEN,
    ) -> *mut ::std::os::raw::c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBvgetallocr(p_ub, fldidocc, cname, occ, usrtype, flags, extralen)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBvgetallocr(
                self.c_ctx_ptr(),
                p_ub,
                fldidocc,
                cname,
                occ,
                usrtype,
                flags,
                extralen,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn cbvgetr(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_long,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::CBvgetr(p_ub, fldidocc, cname, occ, buf, len, usrtype, flags)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OCBvgetr(
                self.c_ctx_ptr(),
                p_ub,
                fldidocc,
                cname,
                occ,
                buf,
                len,
                usrtype,
                flags,
            )
        }
    }

    #[inline]
    pub(crate) unsafe fn _bget_ferror_addr(&self) -> *mut ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::_Bget_Ferror_addr()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::O_Bget_Ferror_addr(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) unsafe fn ndrx_bget_ferror_addr(&self) -> *mut ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_Bget_Ferror_addr()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_Bget_Ferror_addr(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) unsafe fn ndrx_ubf_tls_free(&self, data: *mut ::std::os::raw::c_void) {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_ubf_tls_free(data)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_ubf_tls_free(self.c_ctx_ptr(), data)
        }
    }

    #[inline]
    pub(crate) unsafe fn ndrx_ubf_tls_get(&self) -> *mut ::std::os::raw::c_void {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_ubf_tls_get()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_ubf_tls_get(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) unsafe fn ndrx_ubf_tls_new(
        &self,
        auto_destroy: ::std::os::raw::c_int,
        auto_set: ::std::os::raw::c_int,
    ) -> *mut ::std::os::raw::c_void {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_ubf_tls_new(auto_destroy, auto_set)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_ubf_tls_new(self.c_ctx_ptr(), auto_destroy, auto_set)
        }
    }

    #[inline]
    pub(crate) unsafe fn ndrx_ubf_tls_set(
        &self,
        data: *mut ::std::os::raw::c_void,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_ubf_tls_set(data)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_ubf_tls_set(self.c_ctx_ptr(), data)
        }
    }

    /// List-based alternative for variadic `Bgetrv`.
    #[inline]
    pub(crate) unsafe fn bgetrv(
        &self,
        p_ub: *mut UBFH,
        buf: *mut ::std::os::raw::c_char,
        buflen: *mut BFLDLEN,
        fldidocc: *mut BFLDID,
    ) -> ::std::os::raw::c_int {
        self.bgetr(p_ub, fldidocc, buf, buflen)
    }

    /// List-based alternative for variadic `CBgetrv`.
    #[inline]
    pub(crate) unsafe fn cbgetrv(
        &self,
        p_ub: *mut UBFH,
        buf: *mut ::std::os::raw::c_char,
        buflen: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
        fldidocc: *mut BFLDID,
    ) -> ::std::os::raw::c_int {
        self.cbgetr(p_ub, fldidocc, buf, buflen, usrtype)
    }

    /// List-based alternative for variadic `CBgetallocrv`.
    #[inline]
    pub(crate) unsafe fn cbgetallocrv(
        &self,
        p_ub: *mut UBFH,
        usrtype: ::std::os::raw::c_int,
        extralen: *mut BFLDLEN,
        fldidocc: *mut BFLDID,
    ) -> *mut ::std::os::raw::c_char {
        self.cbgetallocr(p_ub, fldidocc, usrtype, extralen)
    }

    /// List-based alternative for variadic `Bfindrv`.
    #[inline]
    pub(crate) unsafe fn bfindrv(
        &self,
        p_ub: *mut UBFH,
        p_len: *mut BFLDLEN,
        fldidocc: *mut BFLDID,
    ) -> *mut ::std::os::raw::c_char {
        self.bfindr(p_ub, fldidocc, p_len)
    }

    /// List-based alternative for variadic `CBfindrv`.
    #[inline]
    pub(crate) unsafe fn cbfindrv(
        &self,
        p_ub: *mut UBFH,
        len: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
        fldidocc: *mut BFLDID,
    ) -> *mut ::std::os::raw::c_char {
        self.cbfindr(p_ub, fldidocc, len, usrtype)
    }

    /// List-based alternative for variadic `Bpresrv`.
    #[inline]
    pub(crate) unsafe fn bpresrv(
        &self,
        p_ub: *mut UBFH,
        fldidocc: *mut BFLDID,
    ) -> ::std::os::raw::c_int {
        self.bpresr(p_ub, fldidocc)
    }

    /// List-based alternative for variadic `CBvgetrv`.
    #[inline]
    pub(crate) unsafe fn cbvgetrv(
        &self,
        p_ub: *mut UBFH,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        buf: *mut ::std::os::raw::c_char,
        len: *mut BFLDLEN,
        usrtype: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_long,
        fldidocc: *mut BFLDID,
    ) -> ::std::os::raw::c_int {
        self.cbvgetr(p_ub, fldidocc, cname, occ, buf, len, usrtype, flags)
    }

    /// List-based alternative for variadic `Bvnullrv`.
    #[inline]
    pub(crate) unsafe fn bvnullrv(
        &self,
        p_ub: *mut UBFH,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        fldidocc: *mut BFLDID,
    ) -> ::std::os::raw::c_int {
        self.bvnullr(p_ub, fldidocc, cname, occ)
    }

    /// List-based alternative for variadic `CBvgetallocrv`.
    #[inline]
    pub(crate) unsafe fn cbvgetallocrv(
        &self,
        p_ub: *mut UBFH,
        cname: *mut ::std::os::raw::c_char,
        occ: BFLDOCC,
        usrtype: ::std::os::raw::c_int,
        flags: ::std::os::raw::c_long,
        extralen: *mut BFLDLEN,
        fldidocc: *mut BFLDID,
    ) -> *mut ::std::os::raw::c_char {
        self.cbvgetallocr(p_ub, fldidocc, cname, occ, usrtype, flags, extralen)
    }

    /// Build a field id from a Rust field type enum and field number.
    ///
    /// Wraps `Bmkfldid(3)` while avoiding direct use of Enduro/X raw field type
    /// constants. `field_no` is the numeric field number understood by
    /// Enduro/X, typically the value after field-table base expansion.
    #[inline]
    pub fn bmkfldid_typed(&self, field_type: UbfFieldType, field_no: i32) -> i32 {
        unsafe { self.bmkfldid(field_type.as_raw(), field_no as BFLDID) as i32 }
    }
}
