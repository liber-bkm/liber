use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use liber_core::model::Bookmark;
use liber_core::picker::nucleo_filter;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;

pub fn run_tui(targets: Vec<Bookmark>, initial: &str) -> anyhow::Result<Option<Bookmark>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = tui_loop(&mut terminal, targets, initial);
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    targets: Vec<Bookmark>,
    initial: &str,
) -> anyhow::Result<Option<Bookmark>> {
    let mut filter = initial.to_string();
    let mut selected: usize = 0;
    loop {
        let shown: Vec<Bookmark> = nucleo_filter(targets.clone(), &filter);
        if selected >= shown.len() {
            selected = shown.len().saturating_sub(1);
        }
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(8),
                    Constraint::Length(1),
                ])
                .split(f.area());
            let input = Paragraph::new(filter.as_str()).block(
                Block::default().borders(Borders::ALL).title(format!(
                    "pick ({} match{})",
                    shown.len(),
                    if shown.len() == 1 { "" } else { "es" }
                )),
            );
            f.render_widget(input, chunks[0]);
            f.set_cursor_position((chunks[0].x + filter.len() as u16 + 1, chunks[0].y + 1));
            let items: Vec<ListItem> = shown
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    let mut item = ListItem::new(format!(
                        "[{}] {}  {}",
                        &b.uuid.to_string()[..8],
                        b.title,
                        if b.folder.is_empty() {
                            String::new()
                        } else {
                            format!("({})", b.folder)
                        }
                    ));
                    if i == selected {
                        item = item.style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                    item
                })
                .collect();
            let mut state = ListState::default();
            state.select(if shown.is_empty() {
                None
            } else {
                Some(selected)
            });
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            f.render_stateful_widget(list, chunks[1], &mut state);
            let preview = match shown.get(selected) {
                Some(b) => format!(
                    "{}\n{}\n{}\n{}\n{}",
                    b.title,
                    b.url,
                    if b.description.is_empty() {
                        String::new()
                    } else {
                        b.description.clone()
                    },
                    if b.tags.is_empty() {
                        String::new()
                    } else {
                        format!("#{}", b.tags.join(" #"))
                    },
                    if b.folder.is_empty() {
                        String::new()
                    } else {
                        format!("in {}", b.folder)
                    }
                ),
                None => "no matches".to_string(),
            };
            f.render_widget(
                Paragraph::new(preview)
                    .block(Block::default().borders(Borders::ALL).title("preview")),
                chunks[2],
            );
            f.render_widget(
                Paragraph::new("[enter] open  [esc] cancel  [up/down] move"),
                chunks[3],
            );
        })?;
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        match key.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Enter => {
                if shown.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(shown[selected].clone()));
            }
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => {
                if selected + 1 < shown.len() {
                    selected += 1;
                }
            }
            KeyCode::Backspace => {
                filter.pop();
                selected = 0;
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                filter.push(c);
                selected = 0;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(None),
            _ => {}
        }
    }
}

pub fn confirm_numbered(shown: &[Bookmark]) -> anyhow::Result<Option<Bookmark>> {
    let stderr = io::stderr();
    let mut err = stderr.lock();
    for (i, b) in shown.iter().enumerate() {
        writeln!(err, "[{}] {}\n    {}", i + 1, b.title, b.url)?;
    }
    write!(err, "number to pick: ")?;
    err.flush()?;
    drop(err);
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(liber_core::picker::resolve_plain_pick(shown, &line)
        .and_then(|uuid| shown.iter().find(|b| b.uuid == uuid).cloned()))
}
