use zed_extension_api::{
    self as zed, process::Command, LanguageServerId, SlashCommand, SlashCommandOutput,
    SlashCommandOutputSection, Worktree,
};

struct TodoPlusExtension;

impl zed::Extension for TodoPlusExtension {
    fn new() -> Self {
        Self
    }

    fn run_slash_command(
        &self,
        command: SlashCommand,
        args: Vec<String>,
        worktree: Option<&Worktree>,
    ) -> Result<SlashCommandOutput, String> {
        match command.name.as_str() {
            "todo-about" => output(match worktree {
                Some(worktree) => format!(
                    "Todo Plus loaded successfully.\n\nCurrent worktree: `{}`\n\nThis is the minimal safe build used to verify Zed can load the extension without hanging.",
                    worktree.root_path()
                ),
                None => "Todo Plus loaded successfully.\n\nThis is the minimal safe build used to verify Zed can load the extension without hanging.".to_string(),
            }),
            "todo-new" => run_todo_new(args),
            other => Err(format!("unknown slash command: \"{other}\"")),
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &Worktree,
    ) -> Result<Command, String> {
        if language_server_id.as_ref() != "todo-plus-lsp" {
            return Err(format!(
                "unsupported language server: {}",
                language_server_id.as_ref()
            ));
        }

        let binary = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("todo_plus_lsp");
        Ok(Command::new(binary.to_string_lossy().to_string()))
    }
}

fn run_todo_new(args: Vec<String>) -> Result<SlashCommandOutput, String> {
    let text = args.join(" ").trim().to_string();
    if text.is_empty() {
        return Err("usage: /todo-new Buy oat milk".to_string());
    }

    output(format!("- {}", text))
}

fn output(text: String) -> Result<SlashCommandOutput, String> {
    Ok(SlashCommandOutput {
        sections: vec![SlashCommandOutputSection {
            range: (0..text.len()).into(),
            label: "Todo Plus".to_string(),
        }],
        text,
    })
}

zed::register_extension!(TodoPlusExtension);
