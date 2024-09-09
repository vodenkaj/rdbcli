# rdbcli - Rusty Database CLI

`rdbcli` is a command-line interface (CLI) tool for interacting with MongoDB databases, written in Rust. It allows users to connect to a MongoDB instance, switch between databases, and execute queries directly from the terminal.

## Features

- Connect to MongoDB databases.
- Execute queries through an integrated editor.
- Fuzzy search command history.
- Manage multiple database connections.
- Run shell commands dynamically to set connection URIs.

## Installation

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/vodenkaj/rdbcli/releases/download/v0.1.0/rusty-db-cli-installer.sh | sh
```

## Usage
```bash
To run rdbcli, use the following syntax:

rdbcli [DATABASE_URI] [OPTIONS]

Example

# Connect to a MongoDB instance, switch to a database, and run a query
rdbcli mongodb://localhost:27017
:use mydb

# Press `e`, then write your query in the editor
# After closing the editor, the query will be executed
```

Options

    --debug: Enables debug logs that are stored in $HOME/.config/rusty-db-cli/debug.log.
    --disable-command-history: Disables storing of command history into the file located at $HOME/.config/rusty-db-cli/.command_history.txt.

Keybinds

    e - Opens the editor specified by the $EDITOR environment variable, allowing you to write a database query. The query is executed after you save and close the editor.
    r - Runs the last executed database query.
    : - Opens the command line prompt where you can enter commands defined in the Commands section.
    Enter - Opens and transforms the currently selected document in $EDITOR into JSON format for editing or viewing.
    Arrow Up - When in command mode, it will fuzzy search through the command history, allowing you to quickly re-run previous commands.

Commands

    use <database>: Switches to the specified MongoDB database.
    connect <connection uri>: Changes the current MongoDB connection to the specified URI.

You can also use terminal commands to dynamically set the connection URI by using the following syntax:

```bash

connect !(TERMINAL_COMMAND)

For example:


connect !(echo "mongodb://user:password@localhost:27017")
```
