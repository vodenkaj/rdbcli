use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    process, thread,
};

use anyhow::{anyhow, Result};
use crossterm::event;
use ratatui::{
    layout::{Constraint, Layout},
    style::Style,
    widgets::Paragraph,
};
use regex::Regex;

use super::base::{Component, ComponentCreateInfo};
use crate::{
    iterable_enum,
    managers::event_manager::{ConnectionEvent, Event, EventHandler},
    ui::layouts::CLI_ARGS,
    utils::{external_editor::HISTORY_FILE, fuzzy::filter_fuzzy_matches},
};

#[derive(Default, Clone)]
pub enum Severity {
    #[default]
    Normal,
    Info,
    Error,
}

#[derive(Default, Clone)]
pub struct Message {
    pub severity: Severity,
    pub value: String,
}

struct Command {
    kind: CommandKind,
    args: Vec<String>,
}

impl Command {
    pub fn parse(mut parts: Vec<String>) -> anyhow::Result<Self> {
        if parts.is_empty() {
            return Err(anyhow!("Failed to parse command!"));
        }

        let kind = CommandKind::try_from(parts.remove(0))?;
        let args = parts.join(" ");

        match kind {
            CommandKind::Use | CommandKind::Connect => {
                if args.is_empty() {
                    return Err(anyhow!(format!(
                        "Command '{:?}' requires one argument",
                        kind
                    )));
                }

                if let Some(shell_command) = Command::try_parse_shell_command(args.clone()) {
                    return Ok(Command {
                        kind,
                        args: vec![shell_command],
                    });
                }

                Ok(Command {
                    kind,
                    args: vec![args],
                })
            }
            CommandKind::Quit => Ok(Command {
                kind,
                args: Vec::new(),
            }),
        }
    }

    fn try_parse_shell_command(value: String) -> Option<String> {
        let result = Regex::new(r"!\((.*)\)").ok()?.captures(&value);

        if let Some(cmd) = result?.get(1) {
            let output = process::Command::new("zsh")
                .arg("-ci")
                .arg(cmd.as_str())
                .output()
                .ok();
            let result = std::str::from_utf8(&output?.stdout)
                .ok()?
                .trim()
                .to_string();
            return Some(result);
        }

        None
    }
}

iterable_enum!(pub, CommandKind, Use, Connect, Quit);

impl TryFrom<String> for CommandKind {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "use" => Ok(CommandKind::Use),
            "connect" => Ok(CommandKind::Connect),
            "quit" | "q" => Ok(CommandKind::Quit),
            _ => Err(anyhow!("Value is not a valid CommandType")),
        }
    }
}

impl ToString for CommandKind {
    fn to_string(&self) -> String {
        match &self {
            Self::Use => "use".to_string(),
            Self::Connect => "connect".to_string(),
            Self::Quit => "quit".to_string(),
        }
    }
}

pub struct CommandComponent {
    info: ComponentCreateInfo<Message>,
    history: Vec<String>,
    history_index: i32,
    history_filtered: Vec<String>,
}

impl CommandComponent {
    pub fn new(info: ComponentCreateInfo<Message>) -> Self {
        let mut handle = File::open(HISTORY_FILE.to_string()).unwrap();
        let mut buffer = String::new();

        handle.read_to_string(&mut buffer).unwrap();

        let history: Vec<String> = buffer
            .split('\n')
            .collect::<HashSet<&str>>()
            .into_iter()
            .filter_map(|s| {
                if s.is_empty() {
                    return None;
                }
                Some(s.to_string())
            })
            .collect();

        Self {
            info,
            history_filtered: history.clone(),
            history,
            history_index: 0,
        }
    }

    fn refresh_history_filtered(&mut self) {
        self.history_filtered = filter_fuzzy_matches(&self.info.data.value, &self.history);
    }
}

impl Component for CommandComponent {
    fn get_constraint(&self) -> ratatui::prelude::Constraint {
        self.info.constraint
    }

    fn is_visible(&self) -> bool {
        self.info.visible
    }

    fn set_visibility(&mut self, visible: bool) -> bool {
        self.info.visible = visible;
        visible
    }

    fn draw(&mut self, info: super::base::ComponentDrawInfo) {
        let mut style = Style::default();
        if matches!(self.info.data.severity, Severity::Error) {
            style = style.fg(ratatui::style::Color::Red);
        }

        let text_to_render = self.get_text_to_render();
        let (shadow_text_first, shadow_text_rest, _) =
            self.get_shadow_text_to_render().unwrap_or_default();

        let layout_lengths = if shadow_text_first.is_empty() {
            [text_to_render.len() as u16, 0, 0]
        } else {
            [
                text_to_render.len().saturating_sub(3) as u16,
                shadow_text_first.len() as u16,
                shadow_text_rest.len() as u16,
            ]
        };

        let layout = Layout::new(
            ratatui::layout::Direction::Horizontal,
            Constraint::from_lengths(layout_lengths),
        )
        .split(info.area);

        info.frame
            .render_widget(Paragraph::new(text_to_render).style(style), layout[0]);
        info.frame.render_widget(
            Paragraph::new(shadow_text_first).style(
                style
                    .fg(ratatui::style::Color::DarkGray)
                    .bg(ratatui::style::Color::White),
            ),
            layout[1],
        );
        info.frame.render_widget(
            Paragraph::new(shadow_text_rest).style(style.fg(ratatui::style::Color::DarkGray)),
            layout[2],
        )
    }
}

impl CommandComponent {
    fn get_text_to_render(&self) -> String {
        if self.info.is_focused {
            return format!(":{}█", self.info.data.value);
        }

        self.info.data.value.clone()
    }

    fn get_shadow_text_to_render(&self) -> Option<(String, String, String)> {
        if !self.info.is_focused {
            return None;
        }

        let input = &self.info.data.value;

        let kinds = CommandKind::iter()
            .map(|kind| kind.to_string())
            .collect::<Vec<String>>();

        let shadow_text = filter_fuzzy_matches(input, &kinds).first().cloned();

        if let Some(text) = shadow_text {
            if input.len() >= text.len() {
                return None;
            }

            let mut chars = text.chars().skip(input.len());
            let first = chars.next().unwrap().to_string();
            let rest = chars.collect();

            return Some((first, rest, text));
        }

        None
    }
}

impl EventHandler for CommandComponent {
    fn on_event(&mut self, event: &Event) -> Result<()> {
        match event {
            Event::OnMessage(value) => self.info.data = value.clone(),
            Event::OnInput(value) => match value.mode {
                crate::application::Mode::View => {
                    if let event::KeyCode::Char(':') = value.key.code {
                        self.info.is_focused = true;
                        self.info.data = Message::default();
                        self.history_index = 0;
                    }
                }
                crate::application::Mode::Input => match value.key.code {
                    event::KeyCode::Esc => {
                        self.info.data = Message::default();
                        self.history_index = 0;
                        self.info.is_focused = false;
                    }
                    event::KeyCode::Char(value) => {
                        self.info.data.value += &value.to_string();
                        self.history_index = -1;
                    }
                    event::KeyCode::Backspace => {
                        self.info.data.value.pop();
                        self.history_index = -1;
                    }
                    event::KeyCode::Tab => {
                        if let Some((_, _, text)) = self.get_shadow_text_to_render() {
                            self.info.data.value = text;
                        }
                    }
                    event::KeyCode::Up => {
                        if self.history_index == -1 {
                            self.refresh_history_filtered();
                            self.history_index = 0;
                        }

                        if let Some(history) =
                            self.history_filtered.get(self.history_index as usize)
                        {
                            self.info.data.value.clone_from(history);
                            self.history_index += 1;
                        }
                    }
                    event::KeyCode::Down => {
                        if self.history_index == -1 {
                            self.refresh_history_filtered();
                            self.history_index = 0;
                        }

                        if self.history_index != 0 {
                            self.history_index -= 1;

                            if let Some(history) =
                                self.history_filtered.get(self.history_index as usize)
                            {
                                self.info.data.value.clone_from(history);
                            }
                        }
                    }
                    event::KeyCode::Enter => {
                        self.info.is_focused = false;
                        self.history_index = -1;

                        let input_parts: Vec<String> = self
                            .info
                            .data
                            .value
                            .split(' ')
                            .map(|str| str.to_string())
                            .collect();

                        let command = Command::parse(input_parts);

                        let issued_command = self.info.data.value.clone();

                        if !CLI_ARGS.disable_command_history {
                            thread::spawn(move || {
                                let mut handle = OpenOptions::new()
                                    .append(true)
                                    .open(HISTORY_FILE.to_string())
                                    .unwrap();
                                handle
                                    .write_all(format!("{}\n", issued_command).as_bytes())
                                    .unwrap();
                            });
                        }

                        if let Err(err) = command {
                            self.info.data = Message {
                                value: err.to_string(),
                                severity: Severity::Error,
                            }
                        } else if let Ok(command) = command {
                            match command.kind {
                                CommandKind::Use => {
                                    self.info.event_sender.send(Event::OnConnection(
                                        ConnectionEvent::SwitchDatabase(command.args[0].clone()),
                                    ))?;
                                    self.info.data.value = String::new();
                                }
                                CommandKind::Connect => {
                                    self.info.event_sender.send(Event::OnConnection(
                                        ConnectionEvent::Connect(command.args[0].clone()),
                                    ))?;
                                    self.info.data.value = String::new();
                                }
                                CommandKind::Quit => {
                                    self.info.event_sender.send(Event::OnQuit())?;
                                    self.info.data.value = String::new();
                                }
                            }
                        }
                    }
                    _ => {}
                },
            },
            _ => {}
        }
        Ok(())
    }
}
