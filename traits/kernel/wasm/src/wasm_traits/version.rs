use serde_json::Value;

/// UTC date components from SystemTime (no chrono needed).
fn utc_now() -> (u32, u32, u32, u32, u32, u32) {
    // In WASM, SystemTime may not be available. Fall back to js_sys::Date.
    #[cfg(target_arch = "wasm32")]
    {
        let now = js_sys::Date::new_0();
        let year = now.get_utc_full_year() as u32;
        let month = now.get_utc_month() as u32 + 1; // JS months are 0-indexed
        let day = now.get_utc_date() as u32;
        let h = now.get_utc_hours() as u32;
        let m = now.get_utc_minutes() as u32;
        let s = now.get_utc_seconds() as u32;
        (year, month, day, h, m, s)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = dur.as_secs();
        let days = secs / 86400;
        let tod = secs % 86400;
        let h = (tod / 3600) as u32;
        let m = ((tod % 3600) / 60) as u32;
        let s = (tod % 60) as u32;
        let d = days as i64 + 719468;
        let era = if d >= 0 { d } else { d - 146096 } / 146097;
        let doe = (d - era * 146097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = if month <= 2 { y + 1 } else { y };
        (year as u32, month, day, h, m, s)
    }
}

fn yymmdd_now() -> String {
    let (y, mo, d, _, _, _) = utc_now();
    format!("{:02}{:02}{:02}", y % 100, mo, d)
}

fn hhmmss_now() -> String {
    let (_, _, _, h, m, s) = utc_now();
    format!("{:02}{:02}{:02}", h, m, s)
}

/// sys.version(action) — WASM version (no crate::globals, uses js_sys::Date).
pub fn version(args: &[Value]) -> Value {
    let action = args
        .first()
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();

    match action.as_str() {
        "" | "system" => serde_json::json!({
            "name": "traits-wasm",
            "version": env!("CARGO_PKG_VERSION"),
            "runtime": "wasm32",
        }),
        "date" => {
            let d = yymmdd_now();
            serde_json::json!({ "version": d, "date": d, "mode": "date" })
        }
        "hhmmss" => {
            let d = yymmdd_now();
            let t = hhmmss_now();
            serde_json::json!({
                "version": format!("{}.{}", d, t),
                "date": d,
                "suffix": t,
                "mode": "hhmmss",
            })
        }
        _ => {
            let d = yymmdd_now();
            serde_json::json!({ "version": d, "date": d, "mode": "date" })
        }
    }
}
