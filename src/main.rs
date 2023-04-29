use std::fs;
use std::path::{Path, PathBuf};

use chatgpt::prelude::*;
use clap::Parser;
use sha2::Digest;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Arg{
    #[clap(short, long)]
    conversation: Option<String>,
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

fn conversation_name(args: &Arg) -> Result<String> {
    let cwd = std::env::current_dir().unwrap();
    if is_git_repo(&cwd) {
        let conv = match args.conversation.clone() {

            Some(conv) => conv.clone(),
            None => {
                let path = cwd.as_path();
                sha256_hash_string(path.to_str().unwrap())
            }
        };

        if !conv.starts_with(":") {
            return Ok(conv.to_string());
        }
        let path = cwd.as_path();
        let mut project = sha256_hash_string(path.to_str().unwrap());
        project.push_str(&conv);

        Ok(project)
    } else {
        Ok(args.conversation.clone().unwrap_or("default".to_string()))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arg::parse();

    let key = std::env::var("DUCKY_GPT_KEY").unwrap();
    let prompt = args.prompt.join(" ");

    let session = conversation_name(&args)?;

    // Creating a new ChatGPT client.
    // Note that it requires an API key, and uses
    // tokens from your OpenAI API account balance.
    let client = ChatGPT::new(key)?;
    let mut conversation = load_or_start_conversation(&client, &session).await?;

    // // Sending a message and getting the completion
    let response = conversation.send_message(prompt).await?;

    println!("Response: {}", response.message().content);

    if session != "default" {
        store_conversation(&conversation, &session).await?;
    }

    Ok(())
}
