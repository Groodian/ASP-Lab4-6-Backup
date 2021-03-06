mod net;

use crate::net::client::{Client, ClientStop};
use crate::net::message::Message;
use crate::net::messages::{LoginMessage, PublishGlobalChatMessage, PublishPrivateChatMessage};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use std::{error::Error, io};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};

#[derive(Clone)]
enum InputMode {
    Normal,
    Editing,
    Username,
    PrivateUsername,
    PrivateMessaging,
}

pub enum MessageType {
    Public,
    Private,
}

type ConsoleMessage = (MessageType, String);

struct App {
    /// Current value of the input box
    input: String,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    console_messages: Vec<ConsoleMessage>,
}

impl Default for App {
    fn default() -> App {
        App {
            input: String::new(),
            input_mode: InputMode::Username,
            console_messages: Vec::new(),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::default();
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let address = "127.0.0.1:4444"
        .parse()
        .expect("Error while parsing address!");

    let (console_message_sender, console_message_receiver) = channel::<ConsoleMessage>();

    let mut client = Client::new(address, console_message_sender);
    let client_stop: ClientStop = client.get_client_stop();
    let mut private_username: String = String::new();

    print!("\x1B[2J\x1B[1;1H");

    loop {
        terminal.draw(|f| ui(f, &mut app, &console_message_receiver))?;

        if event::poll(Duration::from_secs(0))? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('q') => {
                            client_stop.stop();
                            client.join();
                            return Ok(());
                        }
                        KeyCode::Char('p') => {
                            app.input_mode = InputMode::PrivateUsername;
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            let message: String = app.input.drain(..).collect();

                            if message.starts_with("private ") {
                                let split = message.split(" ").collect::<Vec<&str>>();

                                let private_chat_message = PublishPrivateChatMessage {
                                    to_user_name: split[1].to_string(),
                                    message: split[2..].join(" "),
                                };
                                client.send_message(Message::new(private_chat_message));

                                app.console_messages.push((
                                    MessageType::Private,
                                    format!(
                                        "[PRIVATE] [ME -> {}] {}",
                                        split[1].to_string(),
                                        split[2..].join(" ")
                                    ),
                                ));
                            } else {
                                let global_chat_message = PublishGlobalChatMessage { message };
                                client.send_message(Message::new(global_chat_message));
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                    InputMode::Username => match key.code {
                        KeyCode::Enter => {
                            let name = app.input.drain(..).collect();

                            let login_message = LoginMessage { user_name: name };
                            client.send_message(Message::new(login_message));

                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    InputMode::PrivateUsername => match key.code {
                        KeyCode::Enter => {
                            private_username = app.input.drain(..).collect();

                            app.input_mode = InputMode::PrivateMessaging;
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    },
                    InputMode::PrivateMessaging => match key.code {
                        KeyCode::Enter => {
                            let private_message: String = app.input.drain(..).collect();

                            let message =
                                format!("{} {} {}", "private", private_username, private_message);

                            let split = message.split(" ").collect::<Vec<&str>>();

                            let private_chat_message = PublishPrivateChatMessage {
                                to_user_name: split[1].to_string(),
                                message: split[2..].join(" "),
                            };
                            client.send_message(Message::new(private_chat_message));

                            app.console_messages.push((
                                MessageType::Private,
                                format!(
                                    "[PRIVATE] [ME -> {}] {}",
                                    split[1].to_string(),
                                    split[2..].join(" ")
                                ),
                            ));
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}

fn ui<B: Backend>(
    f: &mut Frame<B>,
    app: &mut App,
    console_message_receiver: &Receiver<ConsoleMessage>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(1),
            ]
            .as_ref(),
        )
        .split(f.size());

    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled(
                    "e",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow),
                ),
                Span::raw(" to start messaging, "),
                Span::styled(
                    "p",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::LightMagenta),
                ),
                Span::raw(" to start private messaging."),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop writing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to send "),
                Span::styled(
                    "public",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow),
                ),
                Span::raw(" message"),
            ],
            Style::default(),
        ),
        InputMode::Username => (
            vec![
                Span::raw("Enter "),
                Span::styled("Username", Style::default().add_modifier(Modifier::BOLD)),
            ],
            Style::default(),
        ),
        InputMode::PrivateUsername => (
            vec![
                Span::raw("Enter "),
                Span::styled(
                    "private ",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::LightMagenta),
                ),
                Span::raw("Username"),
            ],
            Style::default(),
        ),
        InputMode::PrivateMessaging => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop private messaging, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to send "),
                Span::styled(
                    "private ",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::LightMagenta),
                ),
                Span::raw("Message"),
            ],
            Style::default(),
        ),
    };
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[0]);

    let input = Paragraph::new(app.input.as_ref())
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
            InputMode::Username => Style::default().fg(Color::Green),
            InputMode::PrivateUsername => Style::default().fg(Color::LightMagenta),
            InputMode::PrivateMessaging => Style::default().fg(Color::LightMagenta),
        })
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[1]);
    match app.input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[1].x + app.input.len() as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[1].y + 1,
            )
        }
        InputMode::Username => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[1].x + app.input.len() as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[1].y + 1,
            )
        }
        InputMode::PrivateUsername => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[1].x + app.input.len() as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[1].y + 1,
            )
        }
        InputMode::PrivateMessaging => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[1].x + app.input.len() as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[1].y + 1,
            )
        }
    }

    for console_message in console_message_receiver.try_iter() {
        app.console_messages.push(console_message);
    }

    let mut messages: Vec<ListItem> = app
        .console_messages
        .iter()
        .enumerate()
        .map(|(_i, m)| {
            let color = match m.0 {
                MessageType::Public => Color::Yellow,
                MessageType::Private => Color::LightMagenta,
            };

            let content = vec![Spans::from(Span::styled(
                format!("{}", m.1),
                Style::default().fg(color),
            ))];

            ListItem::new(content)
        })
        .collect();

    let mut size = match size() {
        Ok(x) => x,
        Err(_) => (0, 0),
    };

    if size.1 > 9 {
        size.1 -= 10;
    }

    while messages.len() > size.1.into() {
        messages.remove(0);
    }

    let messages = List::new(messages)
        .block(Block::default().borders(Borders::ALL).title("Messages"))
        .style(Style::default().fg(Color::LightCyan));
    f.render_widget(messages, chunks[2]);
}
