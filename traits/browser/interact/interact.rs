use serde_json::{json, Value};
use std::io::Write;
use std::process::Command;

pub fn interact(args: &[Value]) -> Value {
    let url = match args.first().and_then(|v| v.as_str()) {
        Some(u) if u.starts_with("http://") || u.starts_with("https://") => u.to_string(),
        Some(u) => return json!({"ok": false, "error": format!("invalid URL: {}", u)}),
        None => return json!({"ok": false, "error": "url argument is required"}),
    };

    let actions = args.get(1).cloned().unwrap_or_else(|| json!([]));
    let headless = args.get(2).and_then(|v| v.as_bool()).unwrap_or(true);
    let allow_local = args.get(3).and_then(|v| v.as_bool()).unwrap_or(false);

    if !allow_local && is_local_or_private_url(&url) {
      return json!({
        "ok": false,
        "error": "Refusing local/private URL by default for safety. Pass allow_local=true as 4th argument to opt in.",
        "url": url,
        "hint": "browser.interact(url, actions, headless, true)"
      });
    }

    let node_path = match find_node() {
        Some(p) => p,
        None => {
            return json!({
                "ok": false,
                "error": "Node.js not found. Install Node.js (https://nodejs.org/) to use browser.interact."
            })
        }
    };

    let node_modules_path = match find_playwright_node_modules(&node_path) {
        Some(p) => p,
        None => {
            return json!({
                "ok": false,
                "error": "Playwright not found. Install with:\n  npm install playwright\n  npx playwright install chromium"
            })
        }
    };

    let script = build_playwright_script(&url, &actions, headless);
    let script_path = std::env::temp_dir().join(format!(
        "traits-pw-{}.cjs",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros()
    ));

    {
        let mut f = match std::fs::File::create(&script_path) {
            Ok(f) => f,
            Err(e) => return json!({"ok": false, "error": format!("failed to create temp script: {}", e)}),
        };
        if let Err(e) = f.write_all(script.as_bytes()) {
            return json!({"ok": false, "error": format!("failed to write temp script: {}", e)});
        }
    }

    let out = Command::new(&node_path)
        .arg(&script_path)
        .env("NODE_PATH", &node_modules_path)
        .output();

    let _ = std::fs::remove_file(&script_path);

    match out {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);

            if !result.status.success() {
                return json!({
                    "ok": false,
                    "error": format!("Playwright script failed: {}", stderr.trim())
                });
            }

            // Parse the JSON output from the script
            match serde_json::from_str::<Value>(stdout.trim()) {
                Ok(v) => v,
                Err(_) => json!({
                    "ok": false,
                    "error": format!(
                        "script output parse error. stdout: {}, stderr: {}",
                        stdout.trim(),
                        stderr.trim()
                    )
                }),
            }
        }
        Err(e) => json!({"ok": false, "error": format!("Node.js launch failed: {}", e)}),
    }
}

fn find_node() -> Option<String> {
    let candidates = [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
        "/usr/local/nvm/current/bin/node",
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return Some(c.to_string());
        }
    }
    if Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some("node".to_string());
    }
    None
}

fn find_playwright_node_modules(node_path: &str) -> Option<String> {
      let check = Command::new(node_path)
        .args(["-e", "try { process.stdout.write(require.resolve('playwright/package.json')); } catch(e) { process.exit(1); }"])
        .output()
        .ok()?;
      if check.status.success() {
        let pkg_path = String::from_utf8_lossy(&check.stdout).into_owned();
        if let Some(pos) = pkg_path.rfind("/playwright/package.json") {
          return Some(pkg_path[..pos].to_string());
        }
      }
      let mut probe: Vec<String> = vec![
        "/opt/homebrew/lib/node_modules".into(),
        "/usr/local/lib/node_modules".into(),
        "/usr/lib/node_modules".into(),
      ];
      if let Ok(home) = std::env::var("HOME") {
        probe.push(format!("{}/node_modules", home));
        probe.push(format!("{}/.ai/traits/node_modules", home));
      }
      if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd.as_path();
        for _ in 0..6 {
          probe.push(format!("{}/node_modules", dir.display()));
          if let Some(p) = dir.parent() { dir = p; } else { break; }
        }
      }
      for path in &probe {
        if std::path::Path::new(&format!("{}/playwright/package.json", path)).exists()
          || std::path::Path::new(&format!("{}/@playwright/test/package.json", path)).exists()
        {
          return Some(path.clone());
        }
      }
      None
}

    fn is_local_or_private_url(url: &str) -> bool {
      let host = extract_host(url);
      match host {
        Some(h) => is_local_or_private_host(&h),
        None => false,
      }
    }

    fn extract_host(url: &str) -> Option<String> {
      let after_scheme = url.split_once("://")?.1;
      let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
      let no_auth = authority.rsplit('@').next().unwrap_or(authority);

      if no_auth.starts_with('[') {
        let end = no_auth.find(']')?;
        return Some(no_auth[1..end].to_string());
      }

      Some(no_auth.split(':').next().unwrap_or(no_auth).to_string())
    }

    fn is_local_or_private_host(host: &str) -> bool {
      let h = host.trim().to_ascii_lowercase();
      if h.is_empty() {
        return false;
      }

      if matches!(h.as_str(), "localhost" | "127.0.0.1" | "::1" | "0.0.0.0" | "host.docker.internal") {
        return true;
      }

      if let Ok(v4) = h.parse::<std::net::Ipv4Addr>() {
        let o = v4.octets();
        return o[0] == 10
          || (o[0] == 172 && (16..=31).contains(&o[1]))
          || (o[0] == 192 && o[1] == 168)
          || o[0] == 127
          || (o[0] == 169 && o[1] == 254);
      }

      if let Ok(v6) = h.parse::<std::net::Ipv6Addr>() {
        return v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local();
      }

      h.ends_with(".local")
    }

fn build_playwright_script(url: &str, actions: &Value, headless: bool) -> String {
    let actions_json = actions.to_string();
    let headless_str = if headless { "true" } else { "false" };
    let url_escaped = url.replace('\\', "\\\\").replace('`', "\\`");

    format!(
        r#"'use strict';
const {{ chromium }} = require('playwright');

const url = `{url}`;
const headless = {headless};
const actions = {actions_json};

(async () => {{
  let browser;
  const results = [];

  try {{
    browser = await chromium.launch({{ headless }});
    const context = await browser.newContext({{
      viewport: {{ width: 1280, height: 800 }},
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    }});
    const page = await context.newPage();
    await page.goto(url, {{ waitUntil: 'domcontentloaded', timeout: 20000 }});
    await page.waitForLoadState('networkidle', {{ timeout: 5000 }}).catch(() => {{}});

    for (const action of actions) {{
      const result = {{ action: action.type, ok: false }};
      const timeout = action.timeout || 10000;
      try {{
        switch (action.type) {{
          case 'click':
            await page.click(action.selector, {{ timeout }});
            result.ok = true;
            break;
          case 'hover':
            await page.hover(action.selector, {{ timeout }});
            result.ok = true;
            break;
          case 'type':
          case 'fill':
            await page.fill(action.selector, action.value || '', {{ timeout }});
            result.ok = true;
            break;
          case 'press':
            await page.press(action.selector || 'body', action.value || 'Enter', {{ timeout }});
            result.ok = true;
            break;
          case 'select':
            await page.selectOption(action.selector, action.value || '', {{ timeout }});
            result.ok = true;
            break;
          case 'check':
            await page.check(action.selector, {{ timeout }});
            result.ok = true;
            break;
          case 'evaluate': {{
            const v = await page.evaluate(action.script || 'null');
            result.ok = true;
            result.value = v;
            break;
          }}
          case 'screenshot': {{
            const buf = await page.screenshot({{ fullPage: action.full_page || false, type: 'png' }});
            result.ok = true;
            result.value = 'data:image/png;base64,' + buf.toString('base64');
            break;
          }}
          case 'wait':
            if (action.selector) {{
              await page.waitForSelector(action.selector, {{ timeout }});
            }} else {{
              await page.waitForTimeout(parseInt(action.value) || 1000);
            }}
            result.ok = true;
            break;
          case 'navigate':
            await page.goto(action.value || url, {{ waitUntil: 'domcontentloaded', timeout: 20000 }});
            await page.waitForLoadState('networkidle', {{ timeout: 5000 }}).catch(() => {{}});
            result.ok = true;
            break;
          case 'scroll':
            await page.evaluate('window.scrollBy(0,' + (parseInt(action.value) || 500) + ')');
            result.ok = true;
            break;
          case 'get_text':
            result.value = await page.textContent(action.selector, {{ timeout }});
            result.ok = true;
            break;
          case 'get_attr':
            result.value = await page.getAttribute(action.selector, action.value || 'href', {{ timeout }});
            result.ok = true;
            break;
          case 'get_html':
            result.value = await page.innerHTML(action.selector || 'body', {{ timeout }});
            result.ok = true;
            break;
          case 'get_url':
            result.value = page.url();
            result.ok = true;
            break;
          default:
            result.ok = false;
            result.error = 'unknown action: ' + action.type;
        }}
      }} catch (e) {{
        result.ok = false;
        result.error = e.message;
      }}
      results.push(result);
    }}

    const finalBuf = await page.screenshot({{ fullPage: false, type: 'png' }});
    const screenshot = 'data:image/png;base64,' + finalBuf.toString('base64');
    const finalUrl = page.url();
    await browser.close();
    process.stdout.write(JSON.stringify({{ ok: true, url: finalUrl, results, screenshot }}));
  }} catch (e) {{
    if (browser) await browser.close().catch(() => {{}});
    process.stdout.write(JSON.stringify({{ ok: false, error: e.message, results }}));
  }}
}})();
"#,
        url = url_escaped,
        headless = headless_str,
        actions_json = actions_json
    )
}
