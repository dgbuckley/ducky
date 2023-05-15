# ducky

ducky is a CLI GPT chat tool designed to act as a programming rubber ducky.

## Usage

To quickly get started, you first need to add your OpenAI key. Set the
environment variable "DUCKY_GPT_KEY" to your OpenAI key. Once set, you can
then start chatting with ducky. All arguments are passed as the prompt,
so you can run ducky like this: `ducky How do I write a for loop`.

### Examples

Here are some examples to quickly get started. See `ducky --help` for all options.

```sh
# Using a prompt without opening the editor
ducky "Hello, how are you?"

# Opening the editor to enter a prompt
ducky --editor

# Sending a system message and keeping the context
ducky -s "System message" -k "Prompt 1" -k "Prompt 2"

# Setting the default engine for the conversation
ducky --set-engine gpt3.5-turbo "Tell me a joke"

# Adding code from a file to the message
cat src/main.rs | ducky "Fix my code please"
```

### Using Namespaces

When running Ducky in a Git repository, it automatically creates a conversation
specific to that repository. Ducky provides the `-c|--conversation` flag to
specify a particular conversation. For example, you can set up a "git_commit"
conversation using `ducky -c git_commit`.

ducky supports relative conversations within a repository. You can
create a named conversation specific to a git repository by appending the
conversation name with a colon.  For example, `ducky -c :git_commit` would
use a new conversation instead of the global "git_commit" conversation.

### Conversation Context

ducky retains some conversation history in the chat, but only the three most
recent messages and the responses. To permanently keep messages, you can use the
`-k` flag. This ensures that your message remains in the chat history. It
is particularly useful for instructing ChatGPT on how to respond in the
conversation. For example, to set up a programming assistant, you can send
and keep a message like the following:

```sh
ducky -k "You will be assisting me with developing a ChatGPT CLI tool written in Rust. I will ask for your help in several ways:

- When I encounter an error, I will send you the relevant code and the error message. You will respond with suggested fixes and comments for each change. Additionally, you will provide a summary of the fix, explain why the error occurred, and how your fix resolves it.
- I may ask you to review my code and provide suggestions for making it more performant, idiomatic, and clear. Please prioritize suggestions in that order and provide an explanation for each change you suggest.
- I may ask you general Rust questions. Please provide code examples and comments for each line when answering."
```

Now that message will be included at the top of the history used in the
conversation.

### System Messages

You can add system messages in the same way as saving regular messages. Simply
use the `-s` flag, and the message will be sent as the system role and saved
at the top of the conversation history.


Installation

ducky can be installed with either cargo or, if you have nix, from the nix flake.

### Using Cargo

If you have Cargo installed, you can install `ducky` using the following command:

```sh
cargo install
```

### Using Nix

If you prefer using Nix, you can install `ducky` using the following command:

```sh
nix profile install github:dgbuckley/ducky
```
