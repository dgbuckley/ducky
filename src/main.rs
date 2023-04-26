use std::env::args;
use std::fs;
use std::path::PathBuf;

use chatgpt::prelude::*;

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

#[tokio::main]
async fn main() -> Result<()> {
    // Getting the API key here
    let mut arg_it = args();
    let key = arg_it.nth(1).unwrap();
    let prompt = arg_it.collect::<Vec<String>>().join(" ");
    let name = "test";

    // Creating a new ChatGPT client.
    // Note that it requires an API key, and uses
    // tokens from your OpenAI API account balance.
    let client = ChatGPT::new(key)?;
    let mut conversation = load_or_start_conversation(&client, name).await?;

    // // Sending a message and getting the completion
    let response = conversation.send_message(prompt).await?;

    println!("Response: {}", response.message().content);

    store_conversation(&conversation, name).await?;

    Ok(())
}
