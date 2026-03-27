use serde_json::Value;

pub fn yymmdd_now() -> String {
    let (y, mo, d, _, _, _) = kernel_logic::platform::time::now_utc();
    format!("{:02}{:02}{:02}", y % 100, mo, d)
}

pub fn hhmmss_now() -> String {
    let (_, _, _, h, m, s) = kernel_logic::platform::time::now_utc();
    format!("{:02}{:02}{:02}", h, m, s)
}

fn build_date_version() -> Value {
    let yymmdd = yymmdd_now();
    serde_json::json!({
        "version": yymmdd,
        "date": yymmdd,
        "mode": "date",
    })
}

fn build_intraday_version() -> Value {
    let yymmdd = yymmdd_now();
    let suffix = hhmmss_now();
    serde_json::json!({
        "version": format!("{}.{}", yymmdd, suffix),
        "date": yymmdd,
        "suffix": suffix,
        "mode": "hhmmss",
    })
}

/// Trait entry point: version(action)
/// - no args / "system": show trait system version info
/// - "date": generate YYMMDD version string
/// - "hhmmss": generate YYMMDD.HHMMSS version string
pub fn version(args: &[Value]) -> Value {
    let action = args
        .first()
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();

    match action.as_str() {
        "" | "system" => build_system_version(),
        "date" => build_date_version(),
        "hhmmss" => build_intraday_version(),
        _ => build_date_version(),
    }
}

fn build_system_version() -> Value {
    let trait_count = kernel_logic::platform::registry_count();
    #[cfg(not(target_arch = "wasm32"))]
    {
        serde_json::json!({
            "name": "traits",
            "version": env!("TRAITS_BUILD_VERSION"),
            "traits": trait_count,
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        serde_json::json!({
            "name": "traits-wasm",
            "version": env!("TRAITS_BUILD_VERSION"),
            "traits": trait_count,
            "runtime": "wasm32",
        })
    }
}
