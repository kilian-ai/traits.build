use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn chat_learnings(args: &[Value]) -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = args;
        return json!({
            "ok": false,
            "error": "sys.chat_learnings is not available in WASM"
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let workspace_id = match args.first().and_then(Value::as_str).map(str::trim) {
            Some(value) if !value.is_empty() => value,
            _ => return json!({ "ok": false, "error": "workspace_id is required" }),
        };

        let instruction = match args.get(1).and_then(Value::as_str).map(str::trim) {
            Some(value) if !value.is_empty() => value,
            _ => return json!({ "ok": false, "error": "instruction is required" }),
        };

        let output_path = args
            .get(2)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("LEARNINGS.md");

        let base_dir = args.get(3).and_then(Value::as_str).filter(|value| !value.trim().is_empty());
        let method = args.get(4).and_then(Value::as_str).filter(|value| !value.trim().is_empty()).unwrap_or("auto");
        let model = args.get(5).and_then(Value::as_str).filter(|value| !value.trim().is_empty()).unwrap_or("gpt-4o-mini");
        let mock_response = args.get(6).and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty());

        let chat_result = fetch_chat_protocols(workspace_id, method, base_dir);
        if !chat_result.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            return chat_result;
        }

        let extracted = extract_user_comments(&chat_result, workspace_id);
        let output_path = PathBuf::from(output_path);
        let state_path = default_state_path(&output_path);
        let mut state = load_state(&state_path);

        let new_comments: Vec<Comment> = extracted
            .into_iter()
            .filter(|comment| !state.processed_hashes.contains(&comment.hash))
            .collect();

        if new_comments.is_empty() {
            let current_content = fs::read_to_string(&output_path).ok();
            return json!({
                "ok": true,
                "workspace_id": workspace_id,
                "output_path": output_path.display().to_string(),
                "state_path": state_path.display().to_string(),
                "new_comment_count": 0,
                "learning_count": 0,
                "message": "No new user comments matched the incremental scan",
                "current_content": current_content
            });
        }

        let prompt = build_learning_prompt(instruction, &new_comments);
        let response = match mock_response {
            Some(value) => value.to_string(),
            None => match call_llm_openai(&prompt, model) {
                Ok(value) => value,
                Err(error) => {
                    return json!({
                        "ok": false,
                        "workspace_id": workspace_id,
                        "error": error
                    });
                }
            },
        };

        if let Err(error) = append_learnings(&output_path, workspace_id, instruction, &new_comments, &response) {
            return json!({
                "ok": false,
                "workspace_id": workspace_id,
                "error": error
            });
        }

        for comment in &new_comments {
            state.processed_hashes.insert(comment.hash.clone());
        }
        state.last_workspace_id = Some(workspace_id.to_string());

        if let Err(error) = save_state(&state_path, &state) {
            return json!({
                "ok": false,
                "workspace_id": workspace_id,
                "error": error
            });
        }

        let current_content = fs::read_to_string(&output_path).ok();

        json!({
            "ok": true,
            "workspace_id": workspace_id,
            "output_path": output_path.display().to_string(),
            "state_path": state_path.display().to_string(),
            "new_comment_count": new_comments.len(),
            "learning_count": count_markdown_bullets(&response),
            "current_content": current_content,
            "used_mock_response": mock_response.is_some(),
            "model": model,
            "response": response,
            "comments": new_comments.iter().map(Comment::to_json).collect::<Vec<_>>()
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
struct Comment {
    hash: String,
    session_id: String,
    source: String,
    text: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl Comment {
    fn to_json(&self) -> Value {
        json!({
            "hash": self.hash,
            "session_id": self.session_id,
            "source": self.source,
            "text": self.text
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct LearningState {
    processed_hashes: HashSet<String>,
    last_workspace_id: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_chat_protocols(workspace_id: &str, method: &str, base_dir: Option<&str>) -> Value {
    let mut args = vec![
        Value::String(workspace_id.to_string()),
        Value::String(method.to_string()),
    ];

    if let Some(dir) = base_dir {
        args.push(Value::String(dir.to_string()));
    }

    crate::dispatcher::compiled::dispatch("sys.chat_protocols", &args).unwrap_or_else(|| {
        json!({
            "ok": false,
            "error": "sys.chat_protocols is not available"
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_user_comments(chat_result: &Value, workspace_id: &str) -> Vec<Comment> {
    let mut comments = Vec::new();

    if let Some(sessions) = chat_result.pointer("/sources/json/sessions").and_then(Value::as_array) {
        for session in sessions {
            let session_id = session
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("json-session");

            if let Some(requests) = session.pointer("/data/requests").and_then(Value::as_array) {
                for request in requests {
                    if let Some(text) = request.pointer("/message/text").and_then(Value::as_str) {
                        push_comment(&mut comments, workspace_id, session_id, "json", text);
                    }
                }
            }
        }
    }

    if let Some(items) = chat_result.pointer("/sources/state_vscdb/live_history/value/history/copilot").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            if let Some(text) = item.get("inputText").and_then(Value::as_str) {
                push_comment(&mut comments, workspace_id, &format!("live-{}", index), "state_vscdb", text);
            }
        }
    }

    dedupe_comments(comments)
}

#[cfg(not(target_arch = "wasm32"))]
fn push_comment(comments: &mut Vec<Comment>, workspace_id: &str, session_id: &str, source: &str, text: &str) {
    let normalized = text.trim();
    if normalized.is_empty() {
        return;
    }

    let hash = hash_text(&format!("{}\n{}\n{}\n{}", workspace_id, session_id, source, normalized));
    comments.push(Comment {
        hash,
        session_id: session_id.to_string(),
        source: source.to_string(),
        text: normalized.to_string(),
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn dedupe_comments(comments: Vec<Comment>) -> Vec<Comment> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for comment in comments {
        if seen.insert(comment.hash.clone()) {
            deduped.push(comment);
        }
    }

    deduped
}

#[cfg(not(target_arch = "wasm32"))]
fn build_learning_prompt(instruction: &str, comments: &[Comment]) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are maintaining a durable project memory file named LEARNINGS.md.\n");
    prompt.push_str("Extract only stable, reusable instructions or preferences from the user comments below.\n");
    prompt.push_str("Ignore one-off requests, ephemeral debugging chatter, and anything that is not a durable instruction.\n");
    prompt.push_str("Return Markdown only. Use short bullet points. If nothing durable is present, return the exact line: No durable learnings.\n\n");
    prompt.push_str("Instruction field:\n");
    prompt.push_str(instruction);
    prompt.push_str("\n\nNew user comments:\n");

    for comment in comments {
        prompt.push_str(&format!("- [{}:{}] {}\n", comment.source, comment.session_id, comment.text));
    }

    prompt
}

#[cfg(not(target_arch = "wasm32"))]
fn call_llm_openai(prompt: &str, model: &str) -> Result<String, String> {
    let registry = crate::globals::REGISTRY
        .get()
        .cloned()
        .ok_or_else(|| "Registry is not initialized".to_string())?;
    let timeout = crate::globals::CONFIG
        .get()
        .map(|config| config.traits.timeout)
        .unwrap_or(120);
    let dispatcher = crate::dispatcher::Dispatcher::new(registry, timeout);
    let args = vec![
        crate::types::TraitValue::String(prompt.to_string()),
        crate::types::TraitValue::String(model.to_string()),
    ];
    let config = crate::dispatcher::CallConfig::default();

    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("Failed to create runtime for llm/prompt call: {}", error))?;
        runtime.block_on(dispatcher.call("llm/prompt", args, &config))
            .map_err(|error| error.to_string())
    });

    let result = worker
        .join()
        .map_err(|_| "llm/prompt worker thread panicked".to_string())??;

    match result {
        crate::types::TraitValue::String(text) => Ok(text),
        other => Ok(other.to_json().to_string()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn append_learnings(output_path: &Path, workspace_id: &str, instruction: &str, comments: &[Comment], response: &str) -> Result<(), String> {
    if let Some(parent) = output_path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }

    let header = if output_path.exists() {
        String::new()
    } else {
        "# LEARNINGS\n\nDurable user instructions and preferences extracted from chat history.\n\n".to_string()
    };

    let mut section = String::new();
    section.push_str("## Scan\n\n");
    section.push_str(&format!("- Workspace: {}\n", workspace_id));
    section.push_str(&format!("- Instruction: {}\n", instruction));
    section.push_str(&format!("- New comments: {}\n", comments.len()));
    section.push_str("- Source hashes:\n");
    for comment in comments {
        section.push_str(&format!("  - {}\n", comment.hash));
    }
    section.push_str("\n");
    section.push_str(response.trim());
    section.push_str("\n\n");

    let mut content = header;
    content.push_str(&section);

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(output_path)
        .map_err(|error| format!("Failed to open {}: {}", output_path.display(), error))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("Failed to write {}: {}", output_path.display(), error))
}

#[cfg(not(target_arch = "wasm32"))]
fn load_state(path: &Path) -> LearningState {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<LearningState>(&content).ok())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_state(path: &Path, state: &LearningState) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }

    let json = serde_json::to_string_pretty(state)
        .map_err(|error| format!("Failed to serialize learning state: {}", error))?;
    fs::write(path, json)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))
}

#[cfg(not(target_arch = "wasm32"))]
fn default_state_path(output_path: &Path) -> PathBuf {
    let dir = output_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = output_path.file_stem().and_then(|value| value.to_str()).unwrap_or("LEARNINGS");
    dir.join(format!(".{}.state.json", stem.to_ascii_lowercase()))
}

#[cfg(not(target_arch = "wasm32"))]
fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(not(target_arch = "wasm32"))]
fn count_markdown_bullets(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- ") || trimmed.starts_with("* ")
        })
        .count()
}