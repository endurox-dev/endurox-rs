use std::collections::HashSet;

use endurox_rs::{ubf_fields, AtmiCtx, TypedUbf, UbfValue, TPGETANY};

fn main() {
    let rc = match run() {
        Ok(()) => 0,
        Err(msg) => {
            eprintln!("{msg}");
            1
        }
    };
    std::process::exit(rc);
}

fn run() -> Result<(), String> {
    let scenario = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tpcall".to_string());

    let ctx = AtmiCtx::new().map_err(|e| format!("failed to create AtmiCtx: {e}"))?;
    ctx.tpinit().map_err(|e| format!("tpinit failed: {e}"))?;

    let (svc, expected) = match scenario.as_str() {
        "tpcall" => ("RS_IT_ECHO", "RUST-SERVER:HELLO"),
        "tpforward" => ("RS_IT_FORWARD", "RUST-FORWARDED:HELLO"),
        "inner-ubf" => ("RS_IT_INNER_UBF", "RUST-INNER:HELLO-INNER"),
        "tpacall" => ("RS_IT_ECHO", "RUST-SERVER:HELLO"),
        "tpacall-getany" => return run_tpacall_getany(&ctx),
        other => return Err(format!("unknown integration scenario `{other}`")),
    };

    let req_fld = ubf_fields::T_STRING_FLD;
    let rsp_fld = ubf_fields::T_STRING_2_FLD;

    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;

    if scenario == "inner-ubf" {
        let inner_ubf_fld = ubf_fields::T_UBF_FLD;
        let inner_req_fld = ubf_fields::T_STRING_3_FLD;

        let mut inner = ctx
            .tpalloc_ubf(512)
            .map_err(|e| format!("failed to allocate inner UBF: {e}"))?;
        inner
            .bchg(
                inner_req_fld,
                0,
                UbfValue::String("HELLO-INNER".to_string()),
                true,
            )
            .map_err(|e| format!("failed to set inner request field: {e}"))?;
        buf.bchg(inner_ubf_fld, 0, UbfValue::Ubf(inner), true)
            .map_err(|e| format!("failed to set embedded UBF field: {e}"))?;
    } else {
        buf.bchg(req_fld, 0, UbfValue::String("HELLO".to_string()), true)
            .map_err(|e| format!("failed to set request field: {e}"))?;
    }

    if scenario == "tpacall" {
        let mut cd = ctx
            .tpacall(svc, &buf, 0)
            .map_err(|e| format!("tpacall failed: {e}"))?;
        ctx.tpgetrply(&mut cd, &mut buf, 0)
            .map_err(|e| format!("tpgetrply failed: {e}"))?;
    } else {
        ctx.tpcall(svc, &mut buf, 0)
            .map_err(|e| format!("tpcall failed: {e}"))?;
    }

    assert_response(&buf, rsp_fld, expected)?;

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    Ok(())
}

fn run_tpacall_getany(ctx: &AtmiCtx) -> Result<(), String> {
    let rsp_fld = ubf_fields::T_STRING_2_FLD;
    let mut first = build_echo_request(ctx, "FIRST")?;
    let second = build_echo_request(ctx, "SECOND")?;

    let first_cd = ctx
        .tpacall("RS_IT_ECHO", &first, 0)
        .map_err(|e| format!("first tpacall failed: {e}"))?;
    let second_cd = ctx
        .tpacall("RS_IT_ECHO", &second, 0)
        .map_err(|e| format!("second tpacall failed: {e}"))?;

    let mut pending = HashSet::from([first_cd, second_cd]);
    let mut expected = HashSet::from([
        "RUST-SERVER:FIRST".to_string(),
        "RUST-SERVER:SECOND".to_string(),
    ]);

    for _ in 0..2 {
        let mut cd = 0;
        ctx.tpgetrply(&mut cd, &mut first, TPGETANY)
            .map_err(|e| format!("tpgetrply TPGETANY failed: {e}"))?;

        if !pending.remove(&cd) {
            return Err(format!("unexpected async call descriptor returned: {cd}"));
        }

        let rsp = first
            .bget_string(rsp_fld, 0)
            .map_err(|e| format!("failed to read async response field: {e}"))?;
        if !expected.remove(&rsp) {
            return Err(format!("unexpected async response: `{rsp}`"));
        }
    }

    if !pending.is_empty() || !expected.is_empty() {
        return Err(format!(
            "async replies incomplete: pending={pending:?}, expected={expected:?}"
        ));
    }

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    Ok(())
}

fn build_echo_request<'a>(ctx: &'a AtmiCtx, value: &str) -> Result<TypedUbf<'a>, String> {
    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;
    buf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String(value.to_string()),
        true,
    )
    .map_err(|e| format!("failed to set request field: {e}"))?;
    Ok(buf)
}

fn assert_response(buf: &TypedUbf<'_>, rsp_fld: i32, expected: &str) -> Result<(), String> {
    let rsp = buf
        .bget_string(rsp_fld, 0)
        .map_err(|e| format!("failed to read response field: {e}"))?;

    if rsp != expected {
        return Err(format!(
            "unexpected response: expected `{expected}`, got `{rsp}`"
        ));
    }

    Ok(())
}
