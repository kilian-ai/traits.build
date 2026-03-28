/// Shared context-file injection for llm/prompt providers.
///
/// Reads files from a comma-separated list of paths or globs,
/// returns them as XML-wrapped context blocks that can be prepended to prompts.

/// Read context files and return (filename, content) pairs.
/// Accepts a comma-separated list of file paths (absolute or relative to cwd).
/// Supports glob patterns (e.g. "docs/*.md").
pub fn read_context_files(paths_csv: &str, cwd: &str) -> Vec<(String, String)> {
    let base = std::path::Path::new(cwd);
    let mut files: Vec<(String, String)> = Vec::new();

    for pattern in paths_csv.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let abs_pattern = if std::path::Path::new(pattern).is_absolute() {
            pattern.to_string()
        } else {
            base.join(pattern).to_string_lossy().to_string()
        };

        // Try glob expansion first
        let matched: Vec<_> = glob::glob(&abs_pattern)
            .map(|paths| paths.filter_map(|p| p.ok()).collect())
            .unwrap_or_default();

        if matched.is_empty() {
            // Not a glob — try as a literal path
            let p = if std::path::Path::new(pattern).is_absolute() {
                std::path::PathBuf::from(pattern)
            } else {
                base.join(pattern)
            };
            if let Ok(content) = std::fs::read_to_string(&p) {
                let name = p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| pattern.to_string());
                files.push((name, content));
            }
        } else {
            for path in matched {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string());
                    files.push((name, content));
                }
            }
        }
    }

    files
}

/// Format context files into a single XML-wrapped context block.
pub fn format_context(files: &[(String, String)]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let mut out = String::from("<context>\n");
    for (name, content) in files {
        out.push_str(&format!("<file name=\"{}\">\n{}\n</file>\n", name, content));
    }
    out.push_str("</context>\n\n");
    out
}
