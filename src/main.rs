use std::fs::{self, File};
use std::io::{stdin, stdout, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chatgpt::prelude::*;
use clap::Parser;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
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

#[derive(Serialize, Deserialize)]
struct ConversationData {
    model: String,
    history: Vec<chatgpt::types::ChatMessage>,
}

struct ConversationState {
    conversation: Conversation,
    model: ChatGPTEngine,
    name: Option<String>,
}

fn get_config_dir<'a>() -> Result<PathBuf> {
    let config_dir = match BaseDirs::new() {
        Some(base_dirs) => {
            let config_dir_base = base_dirs.config_dir();
            let mut config_dir = PathBuf::from(config_dir_base);
            config_dir.push("ducky");

            if !config_dir.exists() {
                match fs::create_dir_all(&config_dir) {
                    Ok(_) => config_dir,
                    Err(e) => return Err(anyhow!("{}", e)),
                }
            } else {
                config_dir
            }
        }
        None => return Err(anyhow!("Unable to get config directory")),
    };

    Ok(config_dir)
}

impl ConversationState {
    // store will save the conversation data in the config dir if self.name is not None
    fn store(self: &Self) -> Result<()> {
        println!("{:?}", self.name);
        match &self.name {
            Some(name) => {
                let config_dir = get_config_dir()?;
                let mut path = config_dir;
                path.push(name);
                path.set_extension("json");

                let mut file = File::create(path)?;
                let conv = ConversationData {
                    model: self.model.to_string(),
                    history: self.conversation.history.clone(),
                };
                let contents = serde_json::to_string(&conv)?;
                file.write_all(contents.as_bytes())?;
            }
            _ => (),
        }
        Ok(())
    }

    // load will read in an existing conversation
    fn load_from(path: &Path, name: Option<String>, key: &str) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let conv: ConversationData = serde_json::from_str(&contents)?;

        let model = engine_from_str(&conv.model)?;
        let client = ChatGPT::new_with_config(
            key,
            ModelConfigurationBuilder::default()
                .engine(model)
                .build()
                .unwrap(),
        )?;

        let conversation = Conversation::new_with_history(client, conv.history);

        Ok(ConversationState {
            conversation,
            model,
            name,
        })
    }

    // create a new state and initialize the gpt client
    fn create(name: Option<String>, engine: &str, key: &str) -> Result<Self> {
        let model = engine_from_str(engine)?;
        let client = ChatGPT::new_with_config(
            key,
            ModelConfigurationBuilder::default()
                .engine(model)
                .build()
                .unwrap(),
        )?;

        // TODO support a first message
        let conversation = client.new_conversation();
        // let conversation = Conversation::new(client, first_message);

        Ok(ConversationState {
            conversation,
            model,
            name,
        })
    }
}

fn engine_from_str(s: &str) -> Result<ChatGPTEngine> {
    match s {
        "gpt-3.5-turbo" => Ok(ChatGPTEngine::Gpt35Turbo),
        "gpt-3.5-turbo-0301" => Ok(ChatGPTEngine::Gpt35Turbo_0301),
        "gpt-4" => Ok(ChatGPTEngine::Gpt4),
        "gpt-4-32k" => Ok(ChatGPTEngine::Gpt4_32k),
        "gpt-4-0314" => Ok(ChatGPTEngine::Gpt4_0314),
        "gpt-4-32k-0314" => Ok(ChatGPTEngine::Gpt4_32k_0314),
        custom => Err(anyhow!("Invalid model: {}", custom)),
        // custom => Ok(ChatGPTEngine::Custom(custom.clone())),
    }
}

fn start_conversation(name: Option<String>, key: &str, forced: bool) -> Result<ConversationState> {
    if forced {
        return Ok(ConversationState::create(None, "gpt-3.5-turbo", key)?);
    }

    const MODELS: [&'static str; 7] = [
        "default",
        "gpt-3.5-turbo",
        "gpt-3.5-turbo-0301",
        "gpt-4",
        "gpt-4-32k",
        "gpt-4-0314",
        "gpt-4-32k-0314",
    ];

    let mut used_model: String = "".to_string();
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
        stdin().read_line(&mut used_model).unwrap();
        used_model = used_model.replace("\n", "");
        // Check if valid model
        for model in MODELS {
            if model == used_model {
                break 'a;
            }
        }
    }

    let engine = if used_model == "default" {
        engine_from_str("gpt-3.5-turbo")?
    } else {
        engine_from_str(&used_model)?
    };
    let client = ChatGPT::new_with_config(
        key,
        ModelConfigurationBuilder::default()
            .engine(engine)
            .build()
            .unwrap(),
    )?;
    let conv = (&client).new_conversation();
    return Ok(ConversationState {
        conversation: conv,
        model: engine,
        name,
    });
}

async fn load_or_start_conversation(
    key: &str,
    name: Option<String>,
    forced: bool,
) -> Result<ConversationState> {
    if let Some(name) = name {
        let mut config_file_path = get_config_dir()?;
        config_file_path.push(name.to_owned());
        config_file_path.set_extension("json");

        if !config_file_path.parent().unwrap().exists() {
            fs::create_dir_all(&config_file_path.parent().unwrap())?;
        };

        if !config_file_path.exists() {
            let client = start_conversation(Some(name), key, forced)?;
            return Ok(client);
        }

        let conv = ConversationState::load_from(config_file_path.as_path(), Some(name), key)?;
        return Ok(conv);
    }

    return start_conversation(None, key, forced);
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

async fn repl(state: &mut ConversationState) -> Result<()> {
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
