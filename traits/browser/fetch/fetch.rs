use serde_json::{json, Value};
use scraper::{Html, Selector, ElementRef};
use scraper::node::Node;
use std::process::Command;
use std::collections::HashSet;
use regex::Regex;

pub fn fetch(args: &[Value]) -> Value {
    let url = match args.first().and_then(|v| v.as_str()) {
        Some(u) if u.starts_with("http://") || u.starts_with("https://") => u.to_string(),
        Some(u) => return json!({"ok": false, "error": format!("invalid URL (must start with http/https): {}", u)}),
        None => return json!({"ok": false, "error": "url argument is required"}),
    };
    let css_selector = args.get(1).and_then(|v| v.as_str()).filter(|s| !s.is_empty());

    // Try Chrome headless first (renders JS), fall back to curl
    let html = match fetch_html_rendered(&url) {
        Ok(h) if !h.trim().is_empty() && h.trim().len() > 50 => h,
        _ => match fetch_html_curl(&url) {
            Ok(h) => h,
            Err(e) => return json!({"ok": false, "error": e, "url": url}),
        },
    };

    let document = Html::parse_document(&html);
    let title = extract_title(&document);
    let links = extract_links(&document, &url);
    let content = extract_content(&document, css_selector);

    json!({
        "ok": true,
        "url": url,
        "title": title,
        "content": content,
        "links": links
    })
}

fn fetch_html_rendered(url: &str) -> Result<String, String> {
    let chrome = find_chrome_binary().ok_or_else(|| "Chrome not found".to_string())?;
    let out = Command::new(&chrome)
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--disable-software-rasterizer",
            "--dump-dom",
            "--timeout=12000",
            url,
        ])
        .output()
        .map_err(|e| format!("chrome launch failed: {}", e))?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "chrome exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn fetch_html_curl(url: &str) -> Result<String, String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "20",
            "--location",
            "--user-agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            url,
        ])
        .output()
        .map_err(|e| format!("curl not found: {}", e))?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "curl failed ({}): {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub fn find_chrome_binary() -> Option<String> {
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/snap/bin/chromium",
        "/usr/local/bin/chromium",
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return Some(c.to_string());
        }
    }
    for name in &["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"] {
        if Command::new(name)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some((*name).to_string());
        }
    }
    None
}

fn extract_title(doc: &Html) -> String {
    Selector::parse("title")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .map(|el| el.text().collect::<String>().trim().to_string())
        .unwrap_or_default()
}

fn extract_links(doc: &Html, base_url: &str) -> Vec<Value> {
    let Ok(sel) = Selector::parse("a[href]") else {
        return vec![];
    };
    let mut seen = HashSet::new();
    doc.select(&sel)
        .filter_map(|el| {
            let href = el.value().attr("href")?;
            if href.starts_with('#') || href.starts_with("javascript:") || href.is_empty() {
                return None;
            }
            let text = el.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
            let resolved = resolve_url(base_url, href);
            if seen.insert(resolved.clone()) {
                Some(json!({"text": text, "href": resolved}))
            } else {
                None
            }
        })
        .take(60)
        .collect()
}

fn resolve_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if href.starts_with("//") {
        let scheme = if base.starts_with("https://") { "https:" } else { "http:" };
        return format!("{}{}", scheme, href);
    }
    // Extract scheme + host from base
    if let Some(proto_end) = base.find("://") {
        let after_proto = &base[proto_end + 3..];
        let host_end = after_proto
            .find('/')
            .unwrap_or(after_proto.len());
        let origin = &base[..proto_end + 3 + host_end];
        if href.starts_with('/') {
            return format!("{}{}", origin, href);
        }
        // relative path
        let path = &base[proto_end + 3 + host_end..];
        let parent = path.rfind('/').map(|i| &path[..i]).unwrap_or("");
        return format!("{}{}/{}", origin, parent, href.trim_start_matches("./"));
    }
    href.to_string()
}

fn extract_content(doc: &Html, selector: Option<&str>) -> String {
    if let Some(sel_str) = selector {
        if let Ok(s) = Selector::parse(sel_str) {
            if let Some(el) = doc.select(&s).next() {
                let mut out = String::new();
                element_to_markdown(el, &mut out, 0);
                return clean_whitespace(out);
            }
        }
        return format!("(no element found matching selector: {})", sel_str);
    }

    // Auto-select best content container
    for candidate in &[
        "main",
        "article",
        "[role='main']",
        "[role=\"main\"]",
        ".main-content",
        ".post-content",
        ".article-body",
        "#content",
        "#main",
        ".content",
        "body",
    ] {
        if let Ok(s) = Selector::parse(candidate) {
            if let Some(el) = doc.select(&s).next() {
                let mut out = String::new();
                element_to_markdown(el, &mut out, 0);
                let cleaned = clean_whitespace(out);
                if cleaned.len() > 150 {
                    return cleaned;
                }
            }
        }
    }

    // Final fallback: collect all text
    doc.root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn element_to_markdown(el: ElementRef<'_>, out: &mut String, depth: usize) {
    let tag = el.value().name();

    // Skip non-content elements entirely
    match tag {
        "script" | "style" | "noscript" | "head" | "nav" | "footer" | "aside"
        | "form" | "iframe" | "svg" | "canvas" | "video" | "audio" | "template"
        | "meta" | "link" | "input" | "button" | "select" | "textarea" => return,
        _ => {}
    }

    // Block elements that need line breaks
    match tag {
        "h1" => { out.push_str("\n\n# "); render_inline(el, out); out.push('\n'); return; }
        "h2" => { out.push_str("\n\n## "); render_inline(el, out); out.push('\n'); return; }
        "h3" => { out.push_str("\n\n### "); render_inline(el, out); out.push('\n'); return; }
        "h4" => { out.push_str("\n\n#### "); render_inline(el, out); out.push('\n'); return; }
        "h5" => { out.push_str("\n\n##### "); render_inline(el, out); out.push('\n'); return; }
        "h6" => { out.push_str("\n\n###### "); render_inline(el, out); out.push('\n'); return; }
        "br" => { out.push('\n'); return; }
        "hr" => { out.push_str("\n\n---\n\n"); return; }
        "p" => {
            out.push_str("\n\n");
            render_inline(el, out);
            out.push_str("\n\n");
            return;
        }
        "pre" | "code" if tag == "pre" => {
            let lang = el
                .select(&Selector::parse("code[class]").unwrap_or_else(|_| Selector::parse("*").unwrap()))
                .next()
                .and_then(|c| c.value().attr("class"))
                .and_then(|cls| cls.split_whitespace().find(|c| c.starts_with("language-")))
                .map(|c| c.trim_start_matches("language-"))
                .unwrap_or("");
            let text = el.text().collect::<Vec<_>>().join("");
            out.push_str(&format!("\n\n```{}\n{}\n```\n\n", lang, text.trim_end()));
            return;
        }
        "blockquote" => {
            out.push_str("\n\n");
            let mut inner = String::new();
            for child in el.children() {
                if let Some(child_el) = ElementRef::wrap(child) {
                    element_to_markdown(child_el, &mut inner, depth + 1);
                } else if let Node::Text(text) = child.value() {
                    let t: &str = text;
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        inner.push_str(trimmed);
                        inner.push(' ');
                    }
                }
            }
            for line in inner.trim().lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
            return;
        }
        "ul" | "ol" => {
            out.push('\n');
            let mut idx = 1u32;
            for child in el.children() {
                if let Some(child_el) = ElementRef::wrap(child) {
                    if child_el.value().name() == "li" {
                        let indent = "  ".repeat(depth);
                        if tag == "ul" {
                            out.push_str(&format!("{}- ", indent));
                        } else {
                            out.push_str(&format!("{}{}. ", indent, idx));
                            idx += 1;
                        }
                        render_inline(child_el, out);
                        // Recurse into nested lists
                        for nested in child_el.children() {
                            if let Some(nested_el) = ElementRef::wrap(nested) {
                                let n = nested_el.value().name();
                                if n == "ul" || n == "ol" {
                                    out.push('\n');
                                    element_to_markdown(nested_el, out, depth + 1);
                                }
                            }
                        }
                        out.push('\n');
                    }
                }
            }
            out.push('\n');
            return;
        }
        "table" => {
            render_table(el, out);
            return;
        }
        "img" => {
            let alt = el.value().attr("alt").unwrap_or("");
            let src = el.value().attr("src").unwrap_or("");
            if !src.is_empty() && !src.starts_with("data:") {
                out.push_str(&format!("![{}]({})", alt, src));
            }
            return;
        }
        _ => {}
    }

    // Recurse into unknown block elements
    for child in el.children() {
        match child.value() {
            Node::Text(text) => {
                let t: &str = text;
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    out.push_str(trimmed);
                    out.push(' ');
                }
            }
            _ => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    element_to_markdown(child_el, out, depth);
                }
            }
        }
    }

    match tag {
        "div" | "section" | "article" | "main" | "header" | "aside" => {
            out.push_str("\n\n");
        }
        _ => {}
    }
}

fn render_inline(el: ElementRef<'_>, out: &mut String) {
    for child in el.children() {
        match child.value() {
            Node::Text(text) => {
                let t: &str = text;
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    out.push_str(trimmed);
                    out.push(' ');
                }
            }
            _ => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    render_inline_el(child_el, out);
                }
            }
        }
    }
}

fn render_inline_el(el: ElementRef<'_>, out: &mut String) {
    let tag = el.value().name();
    match tag {
        "script" | "style" | "noscript" => return,
        "strong" | "b" => {
            let t = inline_text(el);
            if !t.is_empty() { out.push_str(&format!("**{}** ", t)); }
        }
        "em" | "i" => {
            let t = inline_text(el);
            if !t.is_empty() { out.push_str(&format!("_{}_", t)); }
        }
        "code" => {
            let t = inline_text(el);
            if !t.is_empty() { out.push_str(&format!("`{}`", t)); }
        }
        "a" => {
            let href = el.value().attr("href").unwrap_or("#");
            let text = inline_text(el);
            if text.is_empty() { return; }
            if href == "#" || href.starts_with("javascript:") {
                out.push_str(&text);
            } else {
                out.push_str(&format!("[{}]({})", text, href));
            }
        }
        "img" => {
            let alt = el.value().attr("alt").unwrap_or("");
            let src = el.value().attr("src").unwrap_or("");
            if !src.is_empty() && !src.starts_with("data:") {
                out.push_str(&format!("![{}]({})", alt, src));
            }
        }
        "br" => { out.push('\n'); }
        "span" | "label" | "sup" | "sub" | "small" | "mark" | "abbr" | "cite" | "time" => {
            // transparent inline containers
            render_inline(el, out);
        }
        _ => {
            // unknown inline: just render text
            let t = inline_text(el);
            if !t.is_empty() { out.push_str(&t); out.push(' '); }
        }
    }
}

fn inline_text(el: ElementRef<'_>) -> String {
    el.text()
        .collect::<Vec<_>>()
        .join("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_table(el: ElementRef<'_>, out: &mut String) {
    let Ok(tr_sel) = Selector::parse("tr") else { return; };
    let Ok(th_sel) = Selector::parse("th") else { return; };
    let Ok(td_sel) = Selector::parse("td") else { return; };

    let rows: Vec<Vec<String>> = el
        .select(&tr_sel)
        .map(|row| {
            let cells: Vec<String> = row
                .select(&th_sel)
                .chain(row.select(&td_sel))
                .map(|cell| inline_text(cell))
                .collect();
            cells
        })
        .filter(|r| !r.is_empty())
        .collect();

    if rows.is_empty() { return; }

    out.push_str("\n\n");
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    for (i, row) in rows.iter().enumerate() {
        out.push('|');
        for j in 0..col_count {
            let cell = row.get(j).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!(" {} |", cell));
        }
        out.push('\n');
        if i == 0 {
            out.push('|');
            for _ in 0..col_count {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

fn clean_whitespace(s: String) -> String {
    let re = Regex::new(r"\n{3,}").unwrap();
    re.replace_all(s.trim(), "\n\n").to_string()
}
