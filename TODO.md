# TODO

## Active Task



## Short term goals

- send stdin as "code".
- keep a configurable short history cache in context
- better cli interface, maybe subcommands?

# Ideas


## Longer term goals

- Beutify cli
  - color formatting code
  - paging when output is larger than the terminal
  - better selecting, maybe with: https://docs.rs/dialoguer/latest/dialoguer/
- Spawn a per-conversation daemon
- have api for usage as library
  - Possibly like kakoune, where commands can be sent to a "session" with a certain flag
- ability to look through history of conversation. Pretty print into a pager?
- Add conversation creation option asking for initial promt
  - Options for default, a user created template, or custom prompt
  - If template selected give opprotunity to edit and customize it for conversation