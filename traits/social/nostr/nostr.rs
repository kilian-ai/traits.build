// social.nostr — Pure crypto + manifest helpers for Nostr (NIP-01).
//
// This trait is intentionally stateless. Key storage and relay WebSocket I/O
// live in the JS/SPA layer. The trait exposes:
//
//   keygen                                → { npub, pubkey_hex, nsec }
//   pubkey            <nsec>              → { npub, pubkey_hex }
//   import_nsec       <nsec|hex>          → { npub, pubkey_hex, nsec }
//   encode_npub       <hex>               → { npub }
//   decode_npub       <npub>              → { pubkey_hex }
//   event_id          <pubkey_hex> <created_at> <kind> <tags_json> <content>
//                                         → { id }
//   sign_event        <nsec> <kind> <content> <tags_json> [created_at]
//                                         → { event }
//   verify_event      <event_json>        → { ok, id, pubkey, kind }
//   manifest_build    <vfs_root>          → { content, file_count, total_bytes }
//
// JS stores `nsec` in localStorage (e.g. `traits.secret.NOSTR_NSEC`) and
// passes it explicitly into actions that need it.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

// ─── bech32 ─────────────────────────────────────────────────────────────────

fn bech32_encode(hrp_str: &str, bytes: &[u8]) -> Result<String, String> {
    let hrp = bech32::Hrp::parse(hrp_str).map_err(|e| format!("bad hrp: {e}"))?;
    bech32::encode::<bech32::Bech32>(hrp, bytes).map_err(|e| format!("bech32 encode: {e}"))
}

fn bech32_decode(s: &str, want_hrp: &str) -> Result<Vec<u8>, String> {
    let (hrp, data) = bech32::decode(s).map_err(|e| format!("bech32 decode: {e}"))?;
    if hrp.as_str() != want_hrp {
        return Err(format!("expected hrp {want_hrp}, got {}", hrp.as_str()));
    }
    Ok(data)
}

// ─── crypto ─────────────────────────────────────────────────────────────────

fn random_secret() -> Result<[u8; 32], String> {
    for _ in 0..16 {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|e| format!("rng: {e}"))?;
        if k256::schnorr::SigningKey::from_bytes(&bytes).is_ok() {
            return Ok(bytes);
        }
    }
    Err("could not generate key".into())
}

fn signing_key_from_bytes(secret: &[u8; 32]) -> Result<k256::schnorr::SigningKey, String> {
    k256::schnorr::SigningKey::from_bytes(secret)
        .map_err(|e| format!("invalid secret key: {e}"))
}

fn pubkey_xonly_hex(secret: &[u8; 32]) -> Result<String, String> {
    let sk = signing_key_from_bytes(secret)?;
    Ok(hex::encode(sk.verifying_key().to_bytes()))
}

fn parse_nsec_or_hex(s: &str) -> Result<[u8; 32], String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("nsec or hex required".into());
    }
    let bytes = if s.starts_with("nsec1") {
        bech32_decode(s, "nsec")?
    } else if s.len() == 64 {
        hex::decode(s).map_err(|e| format!("hex: {e}"))?
    } else {
        return Err("expected nsec1... or 64-char hex".into());
    };
    if bytes.len() != 32 {
        return Err("secret key must be 32 bytes".into());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    signing_key_from_bytes(&out)?;
    Ok(out)
}

// ─── NIP-01 event id + signature ────────────────────────────────────────────

fn canonical(pubkey_hex: &str, created_at: i64, kind: i64, tags: &Value, content: &str) -> String {
    let arr = json!([0, pubkey_hex, created_at, kind, tags, content]);
    serde_json::to_string(&arr).unwrap_or_default()
}

fn compute_id(pubkey_hex: &str, created_at: i64, kind: i64, tags: &Value, content: &str) -> [u8; 32] {
    let s = canonical(pubkey_hex, created_at, kind, tags, content);
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let out = h.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&out);
    id
}

fn sign_id(secret: &[u8; 32], id: &[u8; 32]) -> Result<String, String> {
    use k256::schnorr::signature::Signer;
    let sk = signing_key_from_bytes(secret)?;
    let sig: k256::schnorr::Signature = sk.sign(id);
    Ok(hex::encode(sig.to_bytes()))
}

fn verify(pubkey_hex: &str, id: &[u8; 32], sig_hex: &str) -> bool {
    use k256::schnorr::signature::Verifier;
    let pk = match hex::decode(pubkey_hex) {
        Ok(v) if v.len() == 32 => v,
        _ => return false,
    };
    let vk = match k256::schnorr::VerifyingKey::from_bytes(&pk) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let sb = match hex::decode(sig_hex) { Ok(v) => v, Err(_) => return false };
    let sig = match k256::schnorr::Signature::try_from(sb.as_slice()) {
        Ok(s) => s,
        Err(_) => return false,
    };
    vk.verify(id, &sig).is_ok()
}

// ─── time ───────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    let (y, mo, d, h, m, s) = kernel_logic::platform::time::now_utc();
    days_from_civil(y as i32, mo as i32, d as i32) * 86400
        + (h as i64) * 3600
        + (m as i64) * 60
        + (s as i64)
}

fn days_from_civil(y: i32, m: i32, d: i32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as i64 + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era as i64) * 146097 + doe - 719468
}

// ─── helpers ────────────────────────────────────────────────────────────────

fn err(msg: impl Into<String>) -> Value {
    json!({ "ok": false, "error": msg.into() })
}

fn arg<'a>(args: &'a [Value], i: usize) -> &'a str {
    args.get(i).and_then(|v| v.as_str()).unwrap_or("")
}

fn parse_kind(s: &str) -> Result<i64, String> {
    s.trim().parse::<i64>().map_err(|_| format!("invalid kind: {s}"))
}

fn parse_tags(s: &str) -> Result<Value, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(json!([]));
    }
    let v: Value = serde_json::from_str(s).map_err(|e| format!("tags JSON: {e}"))?;
    if !v.is_array() {
        return Err("tags must be a JSON array".into());
    }
    Ok(v)
}

fn npub_of(secret: &[u8; 32]) -> Result<(String, String), String> {
    let pubkey_hex = pubkey_xonly_hex(secret)?;
    let pk_bytes = hex::decode(&pubkey_hex).map_err(|e| format!("hex: {e}"))?;
    let npub = bech32_encode("npub", &pk_bytes)?;
    Ok((npub, pubkey_hex))
}

// ─── actions ────────────────────────────────────────────────────────────────

fn act_keygen() -> Value {
    let secret = match random_secret() { Ok(s) => s, Err(e) => return err(e) };
    let nsec = match bech32_encode("nsec", &secret) { Ok(s) => s, Err(e) => return err(e) };
    let (npub, pubkey_hex) = match npub_of(&secret) { Ok(t) => t, Err(e) => return err(e) };
    json!({ "ok": true, "npub": npub, "pubkey_hex": pubkey_hex, "nsec": nsec })
}

fn act_pubkey(nsec: &str) -> Value {
    let secret = match parse_nsec_or_hex(nsec) { Ok(s) => s, Err(e) => return err(e) };
    let (npub, pubkey_hex) = match npub_of(&secret) { Ok(t) => t, Err(e) => return err(e) };
    json!({ "ok": true, "npub": npub, "pubkey_hex": pubkey_hex })
}

fn act_import_nsec(input: &str) -> Value {
    let secret = match parse_nsec_or_hex(input) { Ok(s) => s, Err(e) => return err(e) };
    let nsec = match bech32_encode("nsec", &secret) { Ok(s) => s, Err(e) => return err(e) };
    let (npub, pubkey_hex) = match npub_of(&secret) { Ok(t) => t, Err(e) => return err(e) };
    json!({ "ok": true, "npub": npub, "pubkey_hex": pubkey_hex, "nsec": nsec })
}

fn act_encode_npub(hex_pk: &str) -> Value {
    let bytes = match hex::decode(hex_pk.trim()) {
        Ok(b) if b.len() == 32 => b,
        _ => return err("expected 64-char hex"),
    };
    match bech32_encode("npub", &bytes) {
        Ok(s) => json!({ "ok": true, "npub": s }),
        Err(e) => err(e),
    }
}

fn act_decode_npub(npub: &str) -> Value {
    match bech32_decode(npub.trim(), "npub") {
        Ok(b) if b.len() == 32 => json!({ "ok": true, "pubkey_hex": hex::encode(b) }),
        Ok(_) => err("npub wrong length"),
        Err(e) => err(e),
    }
}

fn act_event_id(pubkey_hex: &str, created_at_s: &str, kind_s: &str, tags_s: &str, content: &str) -> Value {
    let created_at: i64 = match created_at_s.trim().parse() {
        Ok(n) => n,
        Err(_) => return err("invalid created_at"),
    };
    let kind = match parse_kind(kind_s) { Ok(k) => k, Err(e) => return err(e) };
    let tags = match parse_tags(tags_s) { Ok(t) => t, Err(e) => return err(e) };
    let id = compute_id(pubkey_hex.trim(), created_at, kind, &tags, content);
    json!({ "ok": true, "id": hex::encode(id) })
}

fn act_sign_event(nsec: &str, kind_s: &str, content: &str, tags_s: &str, created_at_s: &str) -> Value {
    let secret = match parse_nsec_or_hex(nsec) { Ok(s) => s, Err(e) => return err(e) };
    let pubkey_hex = match pubkey_xonly_hex(&secret) { Ok(s) => s, Err(e) => return err(e) };
    let kind = match parse_kind(kind_s) { Ok(k) => k, Err(e) => return err(e) };
    let tags = match parse_tags(tags_s) { Ok(t) => t, Err(e) => return err(e) };
    let created_at: i64 = if created_at_s.trim().is_empty() {
        now_secs()
    } else {
        match created_at_s.trim().parse() {
            Ok(n) => n,
            Err(_) => return err("invalid created_at"),
        }
    };
    let id = compute_id(&pubkey_hex, created_at, kind, &tags, content);
    let sig = match sign_id(&secret, &id) { Ok(s) => s, Err(e) => return err(e) };
    let event = json!({
        "id": hex::encode(id),
        "pubkey": pubkey_hex,
        "created_at": created_at,
        "kind": kind,
        "tags": tags,
        "content": content,
        "sig": sig,
    });
    json!({ "ok": true, "event": event })
}

fn act_verify_event(event_json: &str) -> Value {
    let v: Value = match serde_json::from_str(event_json) {
        Ok(v) => v,
        Err(e) => return err(format!("event JSON: {e}")),
    };
    let pubkey = v.get("pubkey").and_then(|x| x.as_str()).unwrap_or("");
    let kind = v.get("kind").and_then(|x| x.as_i64()).unwrap_or(-1);
    let created_at = v.get("created_at").and_then(|x| x.as_i64()).unwrap_or(-1);
    let tags = v.get("tags").cloned().unwrap_or(json!([]));
    let content = v.get("content").and_then(|x| x.as_str()).unwrap_or("");
    let id_field = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
    let sig = v.get("sig").and_then(|x| x.as_str()).unwrap_or("");
    if pubkey.is_empty() || kind < 0 || created_at < 0 || id_field.is_empty() || sig.is_empty() {
        return err("missing fields");
    }
    let computed = compute_id(pubkey, created_at, kind, &tags, content);
    let computed_hex = hex::encode(computed);
    if computed_hex != id_field {
        return json!({ "ok": false, "error": "id mismatch", "computed_id": computed_hex });
    }
    let valid = verify(pubkey, &computed, sig);
    json!({ "ok": valid, "id": id_field, "pubkey": pubkey, "kind": kind })
}

fn act_manifest_build(root: &str) -> Value {
    let root = if root.trim().is_empty() {
        "public".to_string()
    } else {
        root.trim().to_string()
    };
    let list_res = kernel_logic::platform::dispatch(
        "sys.vfs",
        &[json!("list"), json!(&root), json!(true)],
    );
    let list_val = match list_res {
        Some(v) => v,
        None => return err("sys.vfs not available"),
    };
    let entries = list_val
        .get("entries")
        .or_else(|| list_val.get("files"))
        .cloned()
        .unwrap_or(Value::Null);
    let arr = match entries.as_array() {
        Some(a) => a.clone(),
        None => return err(format!("vfs list returned no entries: {}", list_val)),
    };
    let mut files = Vec::new();
    let mut total: u64 = 0;
    for ent in arr {
        let path = ent
            .get("path")
            .and_then(|x| x.as_str())
            .or_else(|| ent.as_str())
            .unwrap_or("")
            .to_string();
        let is_dir = ent
            .get("type")
            .and_then(|x| x.as_str())
            .map(|s| s == "dir")
            .unwrap_or(false);
        if path.is_empty() || is_dir {
            continue;
        }
        let read_res =
            kernel_logic::platform::dispatch("sys.vfs", &[json!("read"), json!(&path)]);
        let content = match read_res
            .as_ref()
            .and_then(|v| v.get("content"))
            .and_then(|x| x.as_str())
        {
            Some(s) => s.to_string(),
            None => continue,
        };
        let mut h = Sha256::new();
        h.update(content.as_bytes());
        let hash = hex::encode(h.finalize());
        let size = content.len() as u64;
        total += size;
        let rel = path
            .strip_prefix(&format!("{}/", root))
            .unwrap_or(path.strip_prefix(&root).unwrap_or(&path))
            .trim_start_matches('/')
            .to_string();
        files.push(json!({ "p": rel, "h": format!("sha256:{}", hash), "s": size }));
    }
    let count = files.len();
    let manifest = json!({ "v": 1, "files": files });
    let content = serde_json::to_string(&manifest).unwrap_or_default();
    json!({
        "ok": true,
        "content": content,
        "file_count": count,
        "total_bytes": total,
    })
}

// ─── dispatch ───────────────────────────────────────────────────────────────

pub fn nostr(args: &[Value]) -> Value {
    let action = arg(args, 0);
    match action {
        "keygen" => act_keygen(),
        "pubkey" => act_pubkey(arg(args, 1)),
        "import_nsec" => act_import_nsec(arg(args, 1)),
        "encode_npub" => act_encode_npub(arg(args, 1)),
        "decode_npub" => act_decode_npub(arg(args, 1)),
        "event_id" => act_event_id(
            arg(args, 1),
            arg(args, 2),
            arg(args, 3),
            arg(args, 4),
            arg(args, 5),
        ),
        "sign_event" => act_sign_event(
            arg(args, 1),
            arg(args, 2),
            arg(args, 3),
            arg(args, 4),
            arg(args, 5),
        ),
        "verify_event" => act_verify_event(arg(args, 1)),
        "manifest_build" => act_manifest_build(arg(args, 1)),
        "" => err("action required (keygen|pubkey|import_nsec|encode_npub|decode_npub|event_id|sign_event|verify_event|manifest_build)"),
        other => err(format!("unknown action: {other}")),
    }
}

pub fn nostr_dispatch(args: &[Value]) -> Value {
    nostr(args)
}
