mod namespace;

use crate::namespace::Namespace;

use std::fs;
use std::io::{stdin, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use bat::PrettyPrinter;
use chatgpt::types::Role;
use clap::Parser;
use directories::BaseDirs;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use sha2::Digest;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Arg {
    #[clap(short, long)]
    /// The conversation to send the chat with
    conversation: Option<String>,

    #[clap(long)]
    /// Open EDITOR to enter the prompt
    editor: bool,

    #[clap(short, long)]
    /// Keep the message to send as context with each prompt
    keep: bool,

    #[clap(long)]
    // TODO add a method to show histor with an index
    /// Keep the message at the specified index
    keep_last: Option<usize>,

    #[clap(short, long)]
    /// History is kept as context as long as this flag is set. Calling without it will immediately clear the persistant session.
    persist: bool,

    #[clap(short, long)]
    /// Open up a repl
    repl: bool,

    #[clap(long)]
    /// Sets the default engine for the conversation
    set_engine: Option<String>,

    /// Send the message as a system message and keep
    #[clap(short, long)]
    system: bool,

    /// The prompt to be sent to GPT
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
// const MODELS: [&'static str; 7] = [
//     "default",
//     "gpt-3.5-turbo",
//     "gpt-3.5-turbo-0301",
//     "gpt-4",
//     "gpt-4-32k",
//     "gpt-4-0314",
//     "gpt-4-32k-0314",
// ];

fn start_conversation(name: Option<String>, key: &str, arg: &Arg) -> Result<Namespace> {
    let state = if let Some(model) = &arg.set_engine {
        Namespace::create(name, &model, key)?
    } else {
        Namespace::create(name, "gpt-3.5-turbo", key)?
    };

    Ok(state)
}

fn load_or_start_conversation(key: &str, name: Option<String>, arg: &Arg) -> Result<Namespace> {
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
                let client = start_conversation(Some(name), key, arg)?;
                return Ok(client);
            }

            let conv = Namespace::load_from(config_file_path.as_path(), Some(name), key)?;
            return Ok(conv);
        }
        None => {
            return start_conversation(None, key, arg);
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
        Ok(args.conversation.clone())
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
        return edit_text("");
    }

    let prompt = args.prompt.join(" ");
    Ok(prompt.trim().to_string())
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

    let key = std::env::var("DUCKY_GPT_KEY").expect("No key found. Please set DUCKY_GPT_KEY.");
    let session = conversation_name(&args)?;

    let mut state = load_or_start_conversation(&key, session, &args)?;

    if args.repl {
        repl(&mut state).await?;
        return Ok(());
    }

    if let Some(keep) = args.keep_last {
        let mut n = keep;
        for i in (0..state.history.len()).rev() {
            if state.history[i].role != Role::User {
                continue;
            }
            if n > 0 {
                n -= n;
                continue;
            }

            state.context.push(state.history[i].clone());
        }

        return Ok(());
    }

    let mut prompt = conversation_prompt(&args)?;

    if !atty::is(atty::Stream::Stdin) {
        let mut stdin_text = String::new();
        stdin().read_to_string(&mut stdin_text)?;

        if prompt.len() > 0 {
            prompt.push_str("\n---\n");
        }
        prompt.push_str(&stdin_text);
    }

    if prompt.len() == 0 {
        eprintln!("No prompt provided, quiting.");
        return Ok(());
    }

    let response = if args.system {
        state
            .send_system_message(prompt, args.keep, args.persist)
            .await?
    } else {
        state.send_message(prompt, args.keep, args.persist).await?
    };

    print_markdown(&response.message().content)?;

    if let Some(name) = &state.name {
        let config_file_path = config_path(&name)?;
        state.store(&config_file_path)?;
    }

    Ok(())
}
