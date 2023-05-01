mod namespace;

use crate::namespace::Namespace;

use std::fs;
use std::io::{stdin, stdout, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bat::PrettyPrinter;
use clap::Parser;
use directories::BaseDirs;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use sha2::Digest;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Arg {
    #[clap(short, long)]
    conversation: Option<String>,
    #[clap(short, long)]
    repl: bool,
    #[clap(short, long)]
    force: bool,
    #[clap(short, long)]
    editor: bool,
    prompt: Vec<String>,
}

fn is_git_repo(dir: &Path) -> bool {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(dir)
        .output()
        .expect("failed to execute git");

    output.status.success()
}

/// Returns the config path and ensures the config
/// directory exists.
pub fn config_path(name: &str) -> Result<PathBuf> {
    let mut config_file_path = match BaseDirs::new() {
        Some(base_dirs) => {
            let config_dir_base = base_dirs.config_dir();
            let mut config_dir = PathBuf::from(config_dir_base);
            config_dir.push("ducky");

            if !config_dir.exists() {
                match std::fs::create_dir_all(&config_dir) {
                    Ok(_) => config_dir,
                    Err(e) => return Err(anyhow!("{}", e)),
                }
            } else {
                config_dir
            }
        }
        None => return Err(anyhow!("Unable to get config directory")),
    };

    config_file_path.push(name.to_owned());
    config_file_path.set_extension("json");

    Ok(config_file_path)
}

// TODO use get https://api.openai.com/v1/models to get a list of models
const MODELS: [&'static str; 7] = [
    "default",
    "gpt-3.5-turbo",
    "gpt-3.5-turbo-0301",
    "gpt-4",
    "gpt-4-32k",
    "gpt-4-0314",
    "gpt-4-32k-0314",
];

fn start_conversation(name: Option<String>, key: &str, forced: bool) -> Result<Namespace> {
    if forced {
        return Ok(Namespace::create(None, "gpt-3.5-turbo", key)?);
    }

    let model_index = dialoguer::Select::new()
        .with_prompt("Specify which model you would like to use")
        .default(0)
        .items(&MODELS)
        .interact()?;

    let model = if model_index == 0 {
        MODELS[1]
    } else {
        MODELS[model_index]
    };

    let state = Namespace::create(name, &model, key)?;

    Ok(state)
}

fn load_or_start_conversation(key: &str, name: Option<String>, forced: bool) -> Result<Namespace> {
    match name {
        Some(name) => {
            let config_file_path = config_path(&name)?;

            if !config_file_path
                .parent()
                .ok_or(anyhow!("Failed to get the parent directory"))?
                .exists()
            {
                fs::create_dir_all(
                    &config_file_path
                        .parent()
                        .ok_or(anyhow!("Failed to get the parent directory"))?,
                )?;
            };

            if !config_file_path.exists() {
                let client = start_conversation(Some(name), key, forced)?;
                return Ok(client);
            }

            let conv = Namespace::load_from(config_file_path.as_path(), Some(name), key)?;
            return Ok(conv);
        }
        None => {
            return start_conversation(None, key, forced);
        }
    }
}

fn sha256_hash_string(input: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let result_str = hex::encode(result);
    result_str
}

fn git_conversation_name() -> Result<String> {
    let cwd = std::env::current_dir().unwrap();
    let output = std::process::Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .expect("failed to execute git");

    let path = String::from_utf8(output.stdout)?.replace("\n", "");

    Ok(sha256_hash_string(&path))
}

fn conversation_name(args: &Arg) -> Result<Option<String>> {
    let cwd = std::env::current_dir().unwrap();
    if is_git_repo(&cwd) {
        let conv = match args.conversation.clone() {
            Some(conv) => conv.clone(),
            None => git_conversation_name()?,
        };

        if !conv.starts_with(":") {
            return Ok(Some(conv.to_string()));
        }
        let mut project = git_conversation_name()?;
        project.push_str(&conv);

        Ok(Some(project))
    } else {
        Ok(None)
    }
}

fn edit_text(text: &str) -> Result<String> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(text.as_bytes())?;

    let path = file.path();
    let path = path.to_str().unwrap();

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let output = std::process::Command::new(editor)
        .arg(path)
        .status()
        .expect("failed to execute editor");

    if !output.success() {
        return Err(anyhow!("Unable to open editor"));
    }

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    if contents.is_empty() {
        return Err(anyhow!("Empty prompt, aborting"));
    }

    Ok(contents)
}

fn conversation_prompt(args: &Arg) -> Result<String> {
    if args.editor {
        let text = edit_text("")?;
        return Ok(text);
    }

    let mut prompt = args.prompt.join(" ");
    if !prompt.is_empty() {
        return Ok(prompt);
    }
    loop {
        print!("Enter Prompt: ");
        _ = stdout().flush();
        stdin().read_line(&mut prompt).unwrap();
        if !prompt.is_empty() {
            return Ok(prompt);
        }
        print!("\n");
    }
}

fn print_markdown(markdown: &str) -> Result<()> {
    let mut printer = PrettyPrinter::new();
    printer.input_from_bytes(markdown.as_bytes());
    printer.language("markdown");

    let theme = std::env::var("DUCKY_THEME")
        .or_else(|_| std::env::var("BAT_THEME"))
        .unwrap_or_else(|_| "base16".to_string());
    printer.theme(theme);

    printer.print()?;
    Ok(())
}

async fn repl(state: &mut Namespace) -> Result<()> {
    let mut editor = DefaultEditor::new()?;

    let mut convo = state.create_conversation();

    println!(
        "Welcome to ChatGPT! Type your message below to start chatting, or type 'exit' to quit."
    );
    loop {
        match editor.readline("> ") {
            Ok(line) => {
                if line == "exit" {
                    break;
                }
                let response = convo.send_message(line.trim()).await?;
                print_markdown(&response.message().content)?;
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    // Ensure we update state history before storing it
    drop(convo);

    if let Some(name) = &state.name {
        let config_file_path = config_path(&name)?;
        state.store(&config_file_path)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arg::parse();

    let key = std::env::var("DUCKY_GPT_KEY")?;
    let session = conversation_name(&args)?;

    // Creating a new ChatGPT client.
    // Note that it requires an API key, and uses
    // tokens from your OpenAI API account balance.
    let mut state = load_or_start_conversation(&key, session, args.force)?;

    if args.repl {
        repl(&mut state).await?;
        return Ok(());
    }

    let prompt = conversation_prompt(&args)?;
    let response = state.send_message(prompt).await?;

    print_markdown(&response.message().content)?;

    if let Some(name) = &state.name {
        let config_file_path = config_path(&name)?;
        state.store(&config_file_path)?;
    }

    Ok(())
}
