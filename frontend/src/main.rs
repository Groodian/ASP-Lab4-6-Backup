mod net;

use crate::net::client::{Client, ClientStop};
use crate::net::message::Message;
use crate::net::messages::{LoginMessage, PublishGlobalChatMessage, PublishPrivateChatMessage};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{error::Error, io};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
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

struct App {
    /// Current value of the input box
    input: String,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: Arc<Mutex<Vec<String>>>,
}

impl Default for App {
    fn default() -> App {
        App {
            input: String::new(),
            input_mode: InputMode::Username,
            messages: Arc::new(Mutex::new(Vec::new())),
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

    let mut client = Client::new(address, Arc::clone(&app.messages));
    let mut client_stop: Option<ClientStop> = None;
    let mut privateUsername: String = String::new();

    print!("\x1B[2J\x1B[1;1H");

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_secs(0))? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('q') => {
                            match client_stop {
                                Some(client_stop) => {
                                    client_stop.stop();
                                    client.join();
                                }
                                None => {}
                            }
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

                                let mut messages_guard = app.messages.lock().unwrap();
                                messages_guard.push(format!(
                                    "[PRIVATE] [ME -> {}] {}",
                                    split[1].to_string(),
                                    split[2..].join(" ")
                                ));
                                drop(messages_guard);
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

                            client_stop = Some(client.connect());
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
                            privateUsername = app.input.drain(..).collect();

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

                            let message = format!("{} {} {}", "private", privateUsername, private_message);

                            let split = message.split(" ").collect::<Vec<&str>>();

                            let private_chat_message = PublishPrivateChatMessage {
                                to_user_name: split[1].to_string(),
                                message: split[2..].join(" "),
                            };
                            client.send_message(Message::new(private_chat_message));

                            let mut messages_guard = app.messages.lock().unwrap();
                            messages_guard.push(format!(
                                "[PRIVATE] [ME -> {}] {}",
                                split[1].to_string(),
                                split[2..].join(" ")
                            ));
                            drop(messages_guard);
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

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
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
                Span::styled("e", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)),
                Span::raw(" to start messaging, "),
                Span::styled("p", Style::default().add_modifier(Modifier::BOLD).fg(Color::LightMagenta)),
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
                Span::styled("public", Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)),
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
                Span::styled("private ", Style::default().add_modifier(Modifier::BOLD).fg(Color::LightMagenta)),
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
                Span::styled("private ", Style::default().add_modifier(Modifier::BOLD).fg(Color::LightMagenta)),
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

    let messages_guard = app.messages.lock().unwrap();
    let messages: Vec<ListItem> = messages_guard
        .iter()
        .enumerate()
        .map(|(_i, m)| {
            let content = vec![Spans::from(Span::raw(format!("{}", m)))];
            ListItem::new(content)
        })
        .collect();
    drop(messages_guard);

    let messages =
        List::new(messages).block(Block::default().borders(Borders::ALL).title("Messages")).style(Style::default().fg(Color::LightCyan));
    f.render_widget(messages, chunks[2]);
}
