use crate::raw::*;
use crate::{raw, AtmiCtx};

/// UBF field type for safe field-id construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UbfFieldType {
    Short,
    Long,
    Char,
    Float,
    Double,
    String,
    Carray,
    Ptr,
    Ubf,
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
    pub unsafe fn b16to32(&self, dest: *mut UBFH, src: *mut UBFH) -> ::std::os::raw::c_int {
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
    pub unsafe fn b32to16(&self, dest: *mut UBFH, src: *mut UBFH) -> ::std::os::raw::c_int {
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
    pub unsafe fn b_error(&self, str_: *mut ::std::os::raw::c_char) {
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
    pub unsafe fn badd(
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
    pub unsafe fn baddfast(
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
    pub unsafe fn badds(
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
    pub unsafe fn balloc(&self, f: BFLDOCC, v: BFLDLEN) -> *mut UBFH {
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
    pub unsafe fn bboolco(&self, expr: *mut ::std::os::raw::c_char) -> *mut ::std::os::raw::c_char {
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
    pub unsafe fn bboolev(
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
    pub unsafe fn bboolpr(&self, tree: *mut ::std::os::raw::c_char, outf: *mut FILE) {
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
    pub unsafe fn bboolprcb(
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
    pub unsafe fn bboolsetcbf(
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
    pub unsafe fn bboolsetcbf2(
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
    pub unsafe fn bchg(
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
    pub unsafe fn bchgs(
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
    pub unsafe fn bcmp(&self, p_ubf1: *mut UBFH, p_ubf2: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bcmp(p_ubf1, p_ubf2)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBcmp(self.c_ctx_ptr(), p_ubf1, p_ubf2)
        }
    }

    #[inline]
    pub unsafe fn bconcat(
        &self,
        p_ub_dst: *mut UBFH,
        p_ub_src: *mut UBFH,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bconcat(p_ub_dst, p_ub_src)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBconcat(self.c_ctx_ptr(), p_ub_dst, p_ub_src)
        }
    }

    #[inline]
    pub unsafe fn bcpy(&self, p_ub_dst: *mut UBFH, p_ub_src: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bcpy(p_ub_dst, p_ub_src)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBcpy(self.c_ctx_ptr(), p_ub_dst, p_ub_src)
        }
    }

    #[inline]
    pub unsafe fn bdel(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bdel(p_ub, bfldid, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBdel(self.c_ctx_ptr(), p_ub, bfldid, occ)
        }
    }

    #[inline]
    pub unsafe fn bdelall(&self, p_ub: *mut UBFH, bfldid: BFLDID) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bdelall(p_ub, bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBdelall(self.c_ctx_ptr(), p_ub, bfldid)
        }
    }

    #[inline]
    pub unsafe fn bdelete(&self, p_ub: *mut UBFH, fldlist: *mut BFLDID) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bdelete(p_ub, fldlist)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBdelete(self.c_ctx_ptr(), p_ub, fldlist)
        }
    }

    #[inline]
    pub unsafe fn becodestr(&self, err: ::std::os::raw::c_int) -> *mut ::std::os::raw::c_char {
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
    pub unsafe fn bextread(&self, p_ub: *mut UBFH, inf: *mut FILE) -> ::std::os::raw::c_int {
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
    pub unsafe fn bextreadcb(
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
    pub unsafe fn bfind(
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
    pub unsafe fn bfindlast(
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
    pub unsafe fn bfindocc(
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
    pub unsafe fn bfindr(
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
    pub unsafe fn bfinds(
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
    pub unsafe fn bflddbadd(
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
    pub unsafe fn bflddbdel(&self, txn: *mut EDB_txn, bfldid: BFLDID) -> ::std::os::raw::c_int {
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
    pub unsafe fn bflddbdrop(&self, txn: *mut EDB_txn) -> ::std::os::raw::c_int {
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
    pub unsafe fn bflddbget(
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
    pub unsafe fn bflddbid(&self, fldname: *mut ::std::os::raw::c_char) -> BFLDID {
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
    pub unsafe fn bflddbload(&self) -> ::std::os::raw::c_int {
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
    pub unsafe fn bflddbname(&self, bfldid: BFLDID) -> *mut ::std::os::raw::c_char {
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
    pub unsafe fn bflddbunlink(&self) -> ::std::os::raw::c_int {
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
    pub unsafe fn bflddbunload(&self) {
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
    pub unsafe fn bfldddbgetenv(
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
    pub unsafe fn bfldid(&self, fldnm: *mut ::std::os::raw::c_char) -> BFLDID {
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
    pub unsafe fn bfldno(&self, bfldid: BFLDID) -> BFLDOCC {
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
    pub unsafe fn bfldtype(&self, bfldid: BFLDID) -> ::std::os::raw::c_int {
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
    pub unsafe fn bfloatev(&self, p_ub: *mut UBFH, tree: *mut ::std::os::raw::c_char) -> f64 {
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
    pub unsafe fn bfname(&self, bfldid: BFLDID) -> *mut ::std::os::raw::c_char {
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
    pub unsafe fn bfprint(&self, p_ub: *mut UBFH, outf: *mut FILE) -> ::std::os::raw::c_int {
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
    pub unsafe fn bfprintcb(
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
    pub unsafe fn bfree(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_int {
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
    pub unsafe fn bget(
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
    pub unsafe fn bgetalloc(
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
    pub unsafe fn bgetlast(
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
    pub unsafe fn bgetr(
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
    pub unsafe fn bgets(
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
    pub unsafe fn bgetsa(
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
    pub unsafe fn bidxused(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_long {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bidxused(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBidxused(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub unsafe fn bindex(&self, p_ub: *mut UBFH, occ: BFLDOCC) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bindex(p_ub, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBindex(self.c_ctx_ptr(), p_ub, occ)
        }
    }

    #[inline]
    pub unsafe fn binit(&self, p_ub: *mut UBFH, len: BFLDLEN) -> ::std::os::raw::c_int {
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
    pub unsafe fn bisubf(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bisubf(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBisubf(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub unsafe fn bjoin(&self, dest: *mut UBFH, src: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bjoin(dest, src)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBjoin(self.c_ctx_ptr(), dest, src)
        }
    }

    #[inline]
    pub unsafe fn blen(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Blen(p_ub, bfldid, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBlen(self.c_ctx_ptr(), p_ub, bfldid, occ)
        }
    }

    #[inline]
    pub unsafe fn bmkfldid(&self, fldtype: ::std::os::raw::c_int, bfldid: BFLDID) -> BFLDID {
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
    pub unsafe fn bneeded(&self, nrfields: BFLDOCC, totsize: BFLDLEN) -> ::std::os::raw::c_long {
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
    pub unsafe fn bnext(
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
    pub unsafe fn bnext2(
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
    pub unsafe fn bnum(&self, p_ub: *mut UBFH) -> BFLDOCC {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bnum(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBnum(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub unsafe fn boccur(&self, p_ub: *mut UBFH, bfldid: BFLDID) -> BFLDOCC {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Boccur(p_ub, bfldid)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBoccur(self.c_ctx_ptr(), p_ub, bfldid)
        }
    }

    #[inline]
    pub unsafe fn bojoin(&self, dest: *mut UBFH, src: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bojoin(dest, src)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBojoin(self.c_ctx_ptr(), dest, src)
        }
    }

    #[inline]
    pub unsafe fn bpres(
        &self,
        p_ub: *mut UBFH,
        bfldid: BFLDID,
        occ: BFLDOCC,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bpres(p_ub, bfldid, occ)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBpres(self.c_ctx_ptr(), p_ub, bfldid, occ)
        }
    }

    #[inline]
    pub unsafe fn bpresr(&self, p_ub: *mut UBFH, fldidocc: *mut BFLDID) -> ::std::os::raw::c_int {
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
    pub unsafe fn bprint(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_int {
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
    pub unsafe fn bproj(&self, p_ub: *mut UBFH, fldlist: *mut BFLDID) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bproj(p_ub, fldlist)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBproj(self.c_ctx_ptr(), p_ub, fldlist)
        }
    }

    #[inline]
    pub unsafe fn bprojcpy(
        &self,
        p_ub_dst: *mut UBFH,
        p_ub_src: *mut UBFH,
        fldlist: *mut BFLDID,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bprojcpy(p_ub_dst, p_ub_src, fldlist)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBprojcpy(self.c_ctx_ptr(), p_ub_dst, p_ub_src, fldlist)
        }
    }

    #[inline]
    pub unsafe fn bread(&self, p_ub: *mut UBFH, inf: *mut FILE) -> ::std::os::raw::c_int {
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
    pub unsafe fn breadcb(
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
    pub unsafe fn brealloc(&self, p_ub: *mut UBFH, f: BFLDOCC, v: BFLDLEN) -> *mut UBFH {
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
    pub unsafe fn brstrindex(&self, p_ub: *mut UBFH, occ: BFLDOCC) -> ::std::os::raw::c_int {
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
    pub unsafe fn bsizeof(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_long {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bsizeof(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBsizeof(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub unsafe fn bstrerror(&self, err: ::std::os::raw::c_int) -> *mut ::std::os::raw::c_char {
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
    pub unsafe fn bsubset(&self, p_ubf1: *mut UBFH, p_ubf2: *mut UBFH) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bsubset(p_ubf1, p_ubf2)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBsubset(self.c_ctx_ptr(), p_ubf1, p_ubf2)
        }
    }

    #[inline]
    pub unsafe fn btreefree(&self, tree: *mut ::std::os::raw::c_char) {
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
    pub unsafe fn btypcvt(
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
    pub unsafe fn btype(&self, bfldid: BFLDID) -> *mut ::std::os::raw::c_char {
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
    pub unsafe fn bunindex(&self, p_ub: *mut UBFH) -> BFLDOCC {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bunindex(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBunindex(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub unsafe fn bunused(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_long {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bunused(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBunused(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub unsafe fn bupdate(
        &self,
        p_ub_dst: *mut UBFH,
        p_ub_src: *mut UBFH,
    ) -> ::std::os::raw::c_int {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bupdate(p_ub_dst, p_ub_src)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBupdate(self.c_ctx_ptr(), p_ub_dst, p_ub_src)
        }
    }

    #[inline]
    pub unsafe fn bused(&self, p_ub: *mut UBFH) -> ::std::os::raw::c_long {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::Bused(p_ub)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::OBused(self.c_ctx_ptr(), p_ub)
        }
    }

    #[inline]
    pub unsafe fn bvcmp(
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
    pub unsafe fn bvcpy(
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
    pub unsafe fn bvextread(
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
    pub unsafe fn bvextreadcb(
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
    pub unsafe fn bvfprint(
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
    pub unsafe fn bvfprintcb(
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
    pub unsafe fn bvftos(
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
    pub unsafe fn bvnext(
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
    pub unsafe fn bvnull(
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
    pub unsafe fn bvnullr(
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
    pub unsafe fn bvoccur(
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
    pub unsafe fn bvopt(
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
    pub unsafe fn bvprint(
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
    pub unsafe fn bvrefresh(&self) {
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
    pub unsafe fn bvselinit(
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
    pub unsafe fn bvsetoccur(
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
    pub unsafe fn bvsinit(
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
    pub unsafe fn bvsizeof(&self, view: *mut ::std::os::raw::c_char) -> ::std::os::raw::c_long {
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
    pub unsafe fn bvstof(
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
    pub unsafe fn bwrite(&self, p_ub: *mut UBFH, outf: *mut FILE) -> ::std::os::raw::c_int {
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
    pub unsafe fn bwritecb(
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
    pub unsafe fn cbadd(
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
    pub unsafe fn cbaddfast(
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
    pub unsafe fn cbchg(
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
    pub unsafe fn cbfind(
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
    pub unsafe fn cbfindocc(
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
    pub unsafe fn cbfindr(
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
    pub unsafe fn cbget(
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
    pub unsafe fn cbgetalloc(
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
    pub unsafe fn cbgetallocr(
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
    pub unsafe fn cbgetr(
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
    pub unsafe fn cbvchg(
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
    pub unsafe fn cbvget(
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
    pub unsafe fn cbvgetalloc(
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
    pub unsafe fn cbvgetallocr(
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
    pub unsafe fn cbvgetr(
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
    pub unsafe fn _bget_ferror_addr(&self) -> *mut ::std::os::raw::c_int {
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
    pub unsafe fn ndrx_bget_ferror_addr(&self) -> *mut ::std::os::raw::c_int {
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
    pub unsafe fn ndrx_ubf_tls_free(&self, data: *mut ::std::os::raw::c_void) {
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
    pub unsafe fn ndrx_ubf_tls_get(&self) -> *mut ::std::os::raw::c_void {
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
    pub unsafe fn ndrx_ubf_tls_new(
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
    pub unsafe fn ndrx_ubf_tls_set(
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
    pub unsafe fn bgetrv(
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
    pub unsafe fn cbgetrv(
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
    pub unsafe fn cbgetallocrv(
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
    pub unsafe fn bfindrv(
        &self,
        p_ub: *mut UBFH,
        p_len: *mut BFLDLEN,
        fldidocc: *mut BFLDID,
    ) -> *mut ::std::os::raw::c_char {
        self.bfindr(p_ub, fldidocc, p_len)
    }

    /// List-based alternative for variadic `CBfindrv`.
    #[inline]
    pub unsafe fn cbfindrv(
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
    pub unsafe fn bpresrv(&self, p_ub: *mut UBFH, fldidocc: *mut BFLDID) -> ::std::os::raw::c_int {
        self.bpresr(p_ub, fldidocc)
    }

    /// List-based alternative for variadic `CBvgetrv`.
    #[inline]
    pub unsafe fn cbvgetrv(
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
    pub unsafe fn bvnullrv(
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
    pub unsafe fn cbvgetallocrv(
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

    /// Safe helper for `Bmkfldid`.
    #[inline]
    pub fn bmkfldid_typed(&self, field_type: UbfFieldType, field_no: i32) -> i32 {
        unsafe { self.bmkfldid(field_type.as_raw(), field_no as BFLDID) as i32 }
    }
}
