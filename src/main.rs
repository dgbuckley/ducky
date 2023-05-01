mod conversation;

use crate::conversation::{config_path, State};

use std::fs;
use std::io::{stdin, stdout, Read, Write};
use std::path::Path;

use anyhow::{anyhow, Result};
use clap::Parser;
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

fn start_conversation(name: Option<String>, key: &str, forced: bool) -> Result<State> {
    if forced {
        return Ok(State::create(None, "gpt-3.5-turbo", key)?);
    }

    let mut used_model = String::new();
    'a: loop {
        // Request desired model
        println!("Specify model for conversation");
        print!("Useable models: ");
        print!("{}", MODELS[0]);
        for model in MODELS.iter().skip(1) {
            print!(", {}", model)
        }
        print!("Enter model: ");
        _ = stdout().flush();
        // Read in user specified model
        stdin().read_line(&mut used_model)?;
        used_model.pop();
        // Check if valid model
        for model in MODELS {
            if model == used_model {
                break 'a;
            }
        }
    }

    used_model = if used_model == "default" {
        "gpt-3.5-turbo".to_string()
    } else {
        used_model
    };
    let state = State::create(name, &used_model, key)?;
    Ok(state)
}

async fn load_or_start_conversation(
    key: &str,
    name: Option<String>,
    forced: bool,
) -> Result<State> {
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

            let conv = State::load_from(config_file_path.as_path(), Some(name), key)?;
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

async fn repl(state: &mut State) -> Result<()> {
    // TODO catch Ctrl-C to save conversation
    loop {
        print!("> ");
        _ = stdout().flush();
        let mut input = String::new();
        stdin().read_line(&mut input)?;
        if input.trim() == "quit" {
            break;
        }
        let response = state.conversation.send_message(input.trim()).await?;
        print!("---\n{}\n---\n", response.message().content);
    }

    state.store()?;

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
    let mut state = load_or_start_conversation(&key, session, args.force).await?;

    if args.repl {
        repl(&mut state).await?;
        return Ok(());
    }

    let prompt = conversation_prompt(&args)?;
    let response = state.conversation.send_message(prompt).await?;

    println!("Response: {}", response.message().content);

    state.store()?;

    Ok(())
}
