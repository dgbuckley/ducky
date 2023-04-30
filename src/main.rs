use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chatgpt::prelude::*;
use clap::Parser;
use sha2::Digest;
use std::io::{stdin, stdout, Read, Write};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Arg {
    #[clap(short, long)]
    conversation: Option<String>,
    #[clap(short, long)]
    repl: bool,
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

async fn load_or_start_conversation(client: &ChatGPT, name: &str) -> Result<Conversation> {
    let mut config_dir_path = PathBuf::new();
    config_dir_path.push(std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let mut home_dir = PathBuf::new();
        home_dir.push(std::env::var("HOME").unwrap());
        home_dir.push(".config");
        home_dir.to_str().unwrap().to_owned()
    }));

    config_dir_path.push("ducky");
    if !config_dir_path.exists() {
        fs::create_dir_all(&config_dir_path)?;
    };

    let mut config_file_path = config_dir_path.clone();
    config_file_path.push(name.to_owned() + ".conf");

    let conversation = match client.restore_conversation_json(config_file_path).await {
        Ok(conv) => conv,
        Err(_) => {
            let conv = client.new_conversation();
            conv
        }
    };

    Ok(conversation)
}

async fn store_conversation(conversation: &Conversation, name: &str) -> Result<()> {
    let mut config_dir_path = PathBuf::new();
    config_dir_path.push(std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let mut home_dir = PathBuf::new();
        home_dir.push(std::env::var("HOME").unwrap());
        home_dir.push(".config");
        home_dir.to_str().unwrap().to_owned()
    }));

    config_dir_path.push("ducky");
    if !config_dir_path.exists() {
        fs::create_dir_all(&config_dir_path)?;
    };

    let mut config_file_path = config_dir_path.clone();
    config_file_path.push(name.to_owned() + ".conf");

    conversation.save_history_json(config_file_path).await?;

    Ok(())
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

fn conversation_name(args: &Arg) -> Result<String> {
    let cwd = std::env::current_dir().unwrap();
    if is_git_repo(&cwd) {
        let conv = match args.conversation.clone() {
            Some(conv) => conv.clone(),
            None => git_conversation_name()?,
        };

        if !conv.starts_with(":") {
            return Ok(conv.to_string());
        }
        let mut project = git_conversation_name()?;
        project.push_str(&conv);

        Ok(project)
    } else {
        Ok(args.conversation.clone().unwrap_or("default".to_string()))
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

async fn repl(client: &ChatGPT, conversation: &mut Conversation, session: String) -> Result<()> {
    // TODO catch Ctrl-C to save conversation
    loop {
        print!("> ");
        _ = stdout().flush();
        let mut input = String::new();
        stdin().read_line(&mut input)?;
        if input.trim() == "quit" {
            break;
        }
        let response = conversation.send_message(input.trim()).await?;
        print!("---\n{}\n---\n", response.message().content);
    }

    store_conversation(&conversation, &session).await?;

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
    let client = ChatGPT::new(key)?;
    let mut conversation = load_or_start_conversation(&client, &session).await?;

    if args.repl {
        repl(&client, &mut conversation, session).await?;
        return Ok(());
    }

    let prompt = conversation_prompt(&args)?;
    let response = conversation.send_message(prompt).await?;

    println!("Response: {}", response.message().content);

    if session != "default" {
        store_conversation(&conversation, &session).await?;
    }

    Ok(())
}
