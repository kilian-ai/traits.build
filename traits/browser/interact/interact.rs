use serde_json::{json, Value};
use std::process::Command;
use std::io::Write;

pub fn interact(args: &[Value]) -> Value {
    let url = match args.first().and_then(|v| v.as_str()) {
        Some(u) if u.starts_with("http://") || u.starts_with("https://") => u.to_string(),
        Some(u) => return json!({"ok": false, "error": format!("invalid URL: {}", u)}),
        None => return json!({"ok": false, "error": "url argument is required"}),
    };

    let actions = args.get(1).cloned().unwrap_or_else(|| json!([]));
    let headless = args.get(2).and_then(|v| v.as_bool()).unwrap_or(true);

    // Check for Node.js
    let node_path = find_node();
    if node_path.is_none() {
        return json!({
            "ok": false,
            "error": "Node.js not found. Install Node.js (https://nodejs.org/) to use browser.interact."
        });
    }
    let node_path = node_path.unwrap();

    // Check for Playwright
    if let Err(e) = check_playwright(&node_path) {
        return json!({
            "ok": false,
            "error": format!(
                "Playwright not found: {}. Install with: npm install -g playwright && npx playwright install chromium",
                e
            )
        });
    }

    // Write the Playwright script to a temp file
    let script = build_playwright_script(&url, &actions, headless);
    let script_path = std::env::temp_dir().join(format!(
        "traits-pw-{}.mjs",
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
        .output();

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

fn check_playwright(node_path: &str) -> Result<(), String> {
    // Try to resolve playwright in global node_modules or via npx
    let check_script = r#"
try {
  require('@playwright/test');
  process.exit(0);
} catch(e1) {
  try {
    require('playwright');
    process.exit(0);
  } catch(e2) {
    process.exit(1);
  }
}
"#;
    let out = Command::new(node_path)
        .args(["-e", check_script])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err("playwright not installed".to_string())
    }
}

fn build_playwright_script(url: &str, actions: &Value, headless: bool) -> String {
    let actions_json = actions.to_string();
    let headless_str = if headless { "true" } else { "false" };
    let url_escaped = url.replace('\\', "\\\\").replace('`', "\\`");

    format!(
        r#"
import {{ chromium }} from 'playwright';
import fs from 'fs';

const url = `{url}`;
const headless = {headless};
const actions = {actions_json};

(async () => {{
  let browser;
  const results = [];
  let screenshotB64 = null;

  try {{
    browser = await chromium.launch({{ headless }});
    const context = await browser.newContext({{
      viewport: {{ width: 1280, height: 800 }},
      userAgent: 'Mozilla/5.0 (compatible; traits-browser/1.0)'
    }});
    const page = await context.newPage();

    // Navigate to starting URL
    await page.goto(url, {{ waitUntil: 'domcontentloaded', timeout: 20000 }});
    await page.waitForLoadState('networkidle', {{ timeout: 5000 }}).catch(() => {{}});

    // Execute each action
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

          case 'select':
            await page.selectOption(action.selector, action.value || '', {{ timeout }});
            result.ok = true;
            break;

          case 'check':
            await page.check(action.selector, {{ timeout }});
            result.ok = true;
            break;

          case 'evaluate':
            const evalResult = await page.evaluate(action.script || 'null');
            result.ok = true;
            result.value = evalResult;
            break;

          case 'screenshot': {{
            const buf = await page.screenshot({{
              fullPage: action.full_page || false,
              type: 'png'
            }});
            const b64 = buf.toString('base64');
            result.ok = true;
            result.value = `data:image/png;base64,${{b64}}`;
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
            await page.evaluate(`window.scrollBy(0, ${{parseInt(action.value) || 500}})`);
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

          default:
            result.ok = false;
            result.error = `unknown action type: ${{action.type}}`;
        }}
      }} catch (e) {{
        result.ok = false;
        result.error = e.message;
      }}
      results.push(result);
    }}

    // Always take a final screenshot of the page state
    const finalBuf = await page.screenshot({{ fullPage: false, type: 'png' }});
    screenshotB64 = `data:image/png;base64,${{finalBuf.toString('base64')}}`;

    // Get current URL (may have navigated)
    const finalUrl = page.url();

    await browser.close();

    console.log(JSON.stringify({{
      ok: true,
      url: finalUrl,
      results,
      screenshot: screenshotB64
    }}));
  }} catch (e) {{
    if (browser) await browser.close().catch(() => {{}});
    console.log(JSON.stringify({{
      ok: false,
      error: e.message,
      results
    }}));
  }}
}})();
"#,
        url = url_escaped,
        headless = headless_str,
        actions_json = actions_json
    )
}
