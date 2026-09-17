use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::process::Command;

struct Server {
    documents: HashMap<String, String>,
    shutdown_requested: bool,
}

impl Server {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            shutdown_requested: false,
        }
    }

    fn handle_message(&mut self, message: Value, stdout: &mut dyn Write) -> io::Result<bool> {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();

        match (method, id) {
            (Some("initialize"), Some(id)) => {
                write_message(
                    stdout,
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "capabilities": {
                                "textDocumentSync": 1,
                                "codeActionProvider": true
                            },
                            "serverInfo": {
                                "name": "todo-plus-lsp"
                            }
                        }
                    }),
                )?;
            }
            (Some("shutdown"), Some(id)) => {
                self.shutdown_requested = true;
                write_message(stdout, json!({"jsonrpc": "2.0", "id": id, "result": Value::Null}))?;
            }
            (Some("textDocument/codeAction"), Some(id)) => {
                let result = self.code_actions(&message["params"]);
                write_message(stdout, json!({"jsonrpc": "2.0", "id": id, "result": result}))?;
            }
            (Some("initialized"), _) => {}
            (Some("exit"), _) => return Ok(false),
            (Some("textDocument/didOpen"), _) => {
                let params = &message["params"]["textDocument"];
                if let (Some(uri), Some(text)) = (
                    params.get("uri").and_then(Value::as_str),
                    params.get("text").and_then(Value::as_str),
                ) {
                    self.documents.insert(uri.to_string(), text.to_string());
                }
            }
            (Some("textDocument/didChange"), _) => {
                self.apply_did_change(&message);
            }
            (_, Some(id)) => {
                write_message(stdout, json!({"jsonrpc": "2.0", "id": id, "result": Value::Null}))?;
            }
            _ => {}
        }

        Ok(true)
    }

    fn apply_did_change(&mut self, message: &Value) {
        let uri = match message["params"]["textDocument"]["uri"].as_str() {
            Some(uri) => uri,
            None => return,
        };
        let Some(changes) = message["params"]["contentChanges"].as_array() else {
            return;
        };

        let doc = self.documents.entry(uri.to_string()).or_default();
        for change in changes {
            if let Some(text) = change.get("text").and_then(Value::as_str) {
                if change.get("range").is_none() {
                    *doc = text.to_string();
                } else if let Some(updated) = apply_range_change(doc, change) {
                    *doc = updated;
                }
            }
        }
    }

    fn code_actions(&self, params: &Value) -> Value {
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(uri) => uri,
            None => return Value::Array(Vec::new()),
        };
        let document = match self.documents.get(uri) {
            Some(document) => document,
            None => return Value::Array(Vec::new()),
        };

        let line_index = params["range"]["start"]["line"]
            .as_u64()
            .map(|line| line as usize)
            .unwrap_or(0);

        let Some(line) = get_line(document, line_index) else {
            return Value::Array(Vec::new());
        };

        let mut actions = Vec::new();
        if let Some(edit) = mark_done_action(uri, line, line_index) {
            actions.push(edit);
        }
        if let Some(edit) = mark_undone_action(uri, line, line_index) {
            actions.push(edit);
        }
        if let Some(edit) = add_todo_here_action(uri, line, line_index) {
            actions.push(edit);
        }
        if let Some(edit) = add_todo_below_action(uri, document, line, line_index) {
            actions.push(edit);
        }
        if let Some(edit) = add_todo_above_action(uri, line, line_index) {
            actions.push(edit);
        }

        Value::Array(actions)
    }
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut server = Server::new();

    while let Some(message) = read_message(&mut reader)? {
        if !server.handle_message(message, &mut writer)? {
            break;
        }
    }

    Ok(())
}

fn read_message(reader: &mut dyn BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        let bytes = reader.read_line(&mut header)?;
        if bytes == 0 {
            return Ok(None);
        }
        if header == "\r\n" {
            break;
        }
        let header = header.trim();
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(length) = content_length else {
        return Ok(None);
    };

    let mut content = vec![0; length];
    reader.read_exact(&mut content)?;
    let value = serde_json::from_slice::<Value>(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(value))
}

fn write_message(writer: &mut dyn Write, value: Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(&value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len())?;
    writer.write_all(&bytes)?;
    writer.flush()
}

fn apply_range_change(document: &str, change: &Value) -> Option<String> {
    let range = change.get("range")?;
    let start = position_to_offset(
        document,
        range.get("start")?.get("line")?.as_u64()? as usize,
        range.get("start")?.get("character")?.as_u64()? as usize,
    )?;
    let end = position_to_offset(
        document,
        range.get("end")?.get("line")?.as_u64()? as usize,
        range.get("end")?.get("character")?.as_u64()? as usize,
    )?;
    let replacement = change.get("text")?.as_str()?;

    let mut updated = String::with_capacity(document.len() + replacement.len());
    updated.push_str(&document[..start]);
    updated.push_str(replacement);
    updated.push_str(&document[end..]);
    Some(updated)
}

fn get_line(document: &str, target: usize) -> Option<&str> {
    document.lines().nth(target).or_else(|| {
        if target == 0 && document.is_empty() {
            Some("")
        } else {
            None
        }
    })
}

fn mark_done_action(uri: &str, line: &str, line_index: usize) -> Option<Value> {
    let task = parse_task(line)?;
    if task.state != TaskState::Unfinished {
        return None;
    }

    let done_at = local_done_timestamp().unwrap_or_else(|| "done".to_string());
    let text = if task.body.is_empty() {
        format!("{}✔ @done({done_at})", task.indent)
    } else {
        format!("{}✔ {} @done({done_at})", task.indent, task.body)
    };

    Some(replace_line_action(
        "Mark Todo Done",
        uri,
        line_index,
        line,
        &text,
    ))
}

fn mark_undone_action(uri: &str, line: &str, line_index: usize) -> Option<Value> {
    let task = parse_task(line)?;
    if task.state != TaskState::Completed {
        return None;
    }

    let body = strip_done_metadata(task.body);
    let text = if body.is_empty() {
        format!("{}☐", task.indent)
    } else {
        format!("{}☐ {}", task.indent, body)
    };

    Some(replace_line_action(
        "Mark Todo Undone",
        uri,
        line_index,
        line,
        &text,
    ))
}

fn add_todo_below_action(uri: &str, document: &str, line: &str, line_index: usize) -> Option<Value> {
    let indent: String = line.chars().take_while(|char| char.is_whitespace()).collect();
    let insert_text = format!("{}☐ ", indent);
    let uses_trailing_newline = document.ends_with('\n');
    let line_count = document.lines().count();
    let is_last_line = line_index + 1 >= line_count.max(1);

    let (position_line, position_character, text) = if is_last_line {
        let suffix = if document.is_empty() || uses_trailing_newline { "" } else { "\n" };
        (
            line_index,
            count_chars(line),
            format!("{suffix}\n{insert_text}"),
        )
    } else {
        (line_index + 1, 0, format!("{insert_text}\n"))
    };

    Some(insert_action(
        "Add Todo Below",
        uri,
        position_line,
        position_character,
        &text,
    ))
}

fn add_todo_above_action(uri: &str, line: &str, line_index: usize) -> Option<Value> {
    let indent: String = line.chars().take_while(|char| char.is_whitespace()).collect();
    Some(insert_action(
        "Add Todo Above",
        uri,
        line_index,
        0,
        &format!("{indent}☐ \n"),
    ))
}

fn add_todo_here_action(uri: &str, line: &str, line_index: usize) -> Option<Value> {
    let indent: String = line.chars().take_while(|char| char.is_whitespace()).collect();
    Some(replace_line_action(
        "Add Todo Here",
        uri,
        line_index,
        line,
        &format!("{indent}☐ "),
    ))
}

fn replace_line_action(title: &str, uri: &str, line_index: usize, old_line: &str, new_line: &str) -> Value {
    let end_character = count_chars(old_line);
    json!({
        "title": title,
        "kind": "quickfix",
        "edit": {
            "changes": {
                uri: [{
                    "range": {
                        "start": {"line": line_index, "character": 0},
                        "end": {"line": line_index, "character": end_character}
                    },
                    "newText": new_line
                }]
            }
        }
    })
}

fn insert_action(title: &str, uri: &str, line: usize, character: usize, new_text: &str) -> Value {
    json!({
        "title": title,
        "kind": "quickfix",
        "edit": {
            "changes": {
                uri: [{
                    "range": {
                        "start": {"line": line, "character": character},
                        "end": {"line": line, "character": character}
                    },
                    "newText": new_text
                }]
            }
        }
    })
}

fn count_chars(text: &str) -> usize {
    text.chars().count()
}

fn position_to_offset(text: &str, line: usize, character: usize) -> Option<usize> {
    let mut offset = 0;
    let mut current_line = 0;

    if line == 0 {
        return char_to_byte_offset(text, character);
    }

    for segment in text.split_inclusive('\n') {
        if current_line == line {
            break;
        }
        offset += segment.len();
        current_line += 1;
    }

    if current_line != line {
        return None;
    }

    char_to_byte_offset(&text[offset..], character).map(|byte_offset| offset + byte_offset)
}

fn char_to_byte_offset(text: &str, character: usize) -> Option<usize> {
    if character == 0 {
        return Some(0);
    }

    let mut count = 0;
    for (index, _) in text.char_indices() {
        if count == character {
            return Some(index);
        }
        count += 1;
    }

    if count == character {
        Some(text.len())
    } else {
        None
    }
}

fn local_done_timestamp() -> Option<String> {
    let output = Command::new("date")
        .arg("+%y-%m-%d %H:%M")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    Some(value.trim().to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskState {
    Unfinished,
    Completed,
}

struct Task<'a> {
    indent: &'a str,
    body: &'a str,
    state: TaskState,
}

fn parse_task(line: &str) -> Option<Task<'_>> {
    let indent_len = line
        .char_indices()
        .find_map(|(index, char)| (!char.is_whitespace()).then_some(index))
        .unwrap_or(line.len());
    let indent = &line[..indent_len];
    let trimmed = &line[indent_len..];

    for marker in ["☐", "-", "✔", "x"] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            let state = if matches!(marker, "✔" | "x") {
                TaskState::Completed
            } else {
                TaskState::Unfinished
            };
            return Some(Task {
                indent,
                body: rest.trim_start(),
                state,
            });
        }
    }

    None
}

fn strip_done_metadata(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    let mut remaining = body;

    while let Some(index) = remaining.find("@done(") {
        result.push_str(&remaining[..index]);
        let after_marker = &remaining[index + "@done(".len()..];
        if let Some(end) = after_marker.find(')') {
            remaining = &after_marker[end + 1..];
        } else {
            remaining = "";
            break;
        }
    }

    result.push_str(remaining);
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unfinished_markers() {
        let task = parse_task("  ☐ Buy milk").expect("task");
        assert_eq!(task.indent, "  ");
        assert_eq!(task.body, "Buy milk");
        assert_eq!(task.state, TaskState::Unfinished);

        let task = parse_task("- Call dentist").expect("task");
        assert_eq!(task.body, "Call dentist");
        assert_eq!(task.state, TaskState::Unfinished);
    }

    #[test]
    fn strips_done_metadata_when_marking_undone() {
        assert_eq!(
            strip_done_metadata("ändra placeholder @done(26-03-06 15:19)"),
            "ändra placeholder"
        );
    }

    #[test]
    fn computes_offsets_for_unicode_lines() {
        let text = "☐ Inställning\n✔ klar";
        let offset = position_to_offset(text, 1, 2).expect("offset");
        assert_eq!(&text[offset..], "klar");
    }
}
