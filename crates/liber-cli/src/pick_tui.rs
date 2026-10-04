use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use liber_core::edit::{ArchiveAction, EditDraft, MarkdownAction};
use liber_core::model::Bookmark;
use liber_core::picker::nucleo_filter;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;

fn with_terminal<T>(
    f: impl FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = f(&mut terminal);
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

pub fn run_tui(targets: Vec<Bookmark>, initial: &str) -> anyhow::Result<Option<Bookmark>> {
    with_terminal(|t| tui_loop(t, targets, initial))
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
                    let id = match b.short_id {
                        Some(n) => n.to_string(),
                        None => b.uuid.to_string()[..8].to_string(),
                    };
                    let mut item = ListItem::new(format!(
                        "[{}] {}  {}{}",
                        id,
                        b.title,
                        if b.folder.is_empty() {
                            String::new()
                        } else {
                            format!("({})", b.folder)
                        },
                        crate::artifact_tags(b)
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
        writeln!(
            err,
            "[{}] {}\n    {}{}",
            i + 1,
            b.title,
            b.url,
            crate::artifact_tags(b)
        )?;
    }
    write!(err, "number to pick: ")?;
    err.flush()?;
    drop(err);
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(liber_core::picker::resolve_plain_pick(shown, &line)
        .and_then(|uuid| shown.iter().find(|b| b.uuid == uuid).cloned()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickAction {
    Open,
    OpenArchive,
    OpenNotes,
    Edit,
}

pub fn action_menu(b: &Bookmark) -> anyhow::Result<Option<PickAction>> {
    with_terminal(|terminal| {
        let mut selected = 0;
        let mut items: Vec<(&str, PickAction)> = vec![("Open in browser", PickAction::Open)];
        if b.archive_file.is_some() {
            items.push(("Open archived copy", PickAction::OpenArchive));
        }
        if b.markdown_file.is_some() {
            items.push(("Show notes", PickAction::OpenNotes));
        }
        items.push(("Edit bookmark", PickAction::Edit));
        loop {
            terminal.draw(|f| {
                let area = centered_rect(44, 8, f.area());
                let id = b
                    .short_id
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| b.uuid.to_string()[..8].to_string());
                let rows: Vec<ListItem> = items
                    .iter()
                    .enumerate()
                    .map(|(i, (label, _))| {
                        let mut item = ListItem::new((*label).to_string());
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
                state.select(Some(selected));
                f.render_stateful_widget(
                    List::new(rows).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!("[{id}] {}", b.title)),
                    ),
                    area,
                    &mut state,
                );
            })?;
            if !event::poll(std::time::Duration::from_millis(100))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Enter => {
                    return Ok(items.get(selected).map(|(_, action)| *action));
                }
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => {
                    if selected + 1 < items.len() {
                        selected += 1;
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None)
                }
                _ => {}
            }
        }
    })
}

fn centered_rect(w: u16, h: u16, area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    ratatui::layout::Rect {
        x,
        y,
        width: w.min(area.width),
        height: h.min(area.height),
    }
}

fn field_index(row: EditRow) -> Option<usize> {
    match row {
        EditRow::Title => Some(0),
        EditRow::Url => Some(1),
        EditRow::Description => Some(2),
        EditRow::Tags => Some(3),
        EditRow::Folder => Some(4),
        _ => None,
    }
}

const BACKENDS: &[&str] = &["auto", "builtin", "browser", "single-file", "monolith"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditRow {
    Title,
    Url,
    Description,
    Tags,
    Folder,
    Markdown,
    Archive,
    Backend,
    Attach,
    Detach,
}

const EDIT_ROWS: &[EditRow] = &[
    EditRow::Title,
    EditRow::Url,
    EditRow::Description,
    EditRow::Tags,
    EditRow::Folder,
    EditRow::Markdown,
    EditRow::Archive,
    EditRow::Backend,
    EditRow::Attach,
    EditRow::Detach,
];

fn row_label(row: EditRow) -> &'static str {
    match row {
        EditRow::Title => "Title      ",
        EditRow::Url => "URL        ",
        EditRow::Description => "Description",
        EditRow::Tags => "Tags       ",
        EditRow::Folder => "Folder     ",
        EditRow::Markdown => "Markdown   ",
        EditRow::Archive => "Archive    ",
        EditRow::Backend => "Backend    ",
        EditRow::Attach => "Attach     ",
        EditRow::Detach => "Detach     ",
    }
}

pub fn run_edit_tui(b: &Bookmark) -> anyhow::Result<Option<EditDraft>> {
    with_terminal(|terminal| {
        let mut fields = [
            b.title.clone(),
            b.url.clone(),
            b.description.clone(),
            b.tags.join(" "),
            b.folder.clone(),
        ];
        let mut markdown_add = false;
        let mut markdown_remove = false;
        let mut archive_add = false;
        let mut archive_remove = false;
        let mut backend_idx = 0;
        let mut attach: Vec<std::path::PathBuf> = Vec::new();
        let mut detach: Vec<String> = Vec::new();
        let mut focus: usize = 0;
        let mut cursor: usize = usize::MAX;
        let mut prompt: Option<(String, String)> = None;
        let mut message = String::new();

        let md_state = if b.markdown_file.is_some() {
            "present"
        } else {
            "absent"
        };
        let arch_state = if b.archive_file.is_some() {
            "present"
        } else {
            "absent"
        };

        loop {
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(12), Constraint::Length(3), Constraint::Length(1)])
                    .split(f.area());
                let mut lines: Vec<ratatui::text::Line> = Vec::new();
                for (i, row) in EDIT_ROWS.iter().enumerate() {
                    let value = match row {
                        EditRow::Title
                        | EditRow::Url
                        | EditRow::Description
                        | EditRow::Tags
                        | EditRow::Folder => fields[field_index(*row).unwrap()].clone(),
                        EditRow::Markdown => {
                            if markdown_remove {
                                "remove".to_string()
                            } else if markdown_add || b.markdown_file.is_some() {
                                format!("present ({md_state})")
                            } else {
                                "absent [space] add".to_string()
                            }
                        }
                        EditRow::Archive => {
                            if archive_remove {
                                "remove".to_string()
                            } else if archive_add || b.archive_file.is_some() {
                                format!("present ({arch_state})")
                            } else {
                                "absent [space] add".to_string()
                            }
                        }
                        EditRow::Backend => BACKENDS[backend_idx].to_string(),
                        EditRow::Attach => {
                            if attach.is_empty() {
                                "[a] add by path".to_string()
                            } else {
                                format!("{} queued", attach.len())
                            }
                        }
                        EditRow::Detach => {
                            let names: Vec<String> =
                                b.attachments.iter().map(|a| a.name.clone()).collect();
                            format!(
                                "{} [d] remove by name/#{}",
                                if names.is_empty() {
                                    "none".to_string()
                                } else {
                                    names.join(", ")
                                },
                                if detach.is_empty() {
                                    String::new()
                                } else {
                                    format!(" queued: {}", detach.join(","))
                                }
                            )
                        }
                    };
                    let style = if i == focus && prompt.is_none() {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(format!("{} ", row_label(*row)), style),
                        ratatui::text::Span::raw(value),
                    ]));
                }
                f.render_widget(
                    Paragraph::new(lines).block(
                        Block::default().borders(Borders::ALL).title(format!(
                            "edit [{}] {}",
                            b.short_id
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| b.uuid.to_string()[..8].to_string()),
                            b.title
                        )),
                    ),
                    chunks[0],
                );
                let prompt_line = match &prompt {
                    Some((label, buf)) => format!("{label}: {buf}_"),
                    None => {
                        if message.is_empty() {
                            "[tab] field  [space] toggle  [a]/[d] attach/detach  [ctrl+s] save  [esc] cancel".to_string()
                        } else {
                            message.clone()
                        }
                    }
                };
                f.render_widget(Paragraph::new(prompt_line), chunks[1]);
                f.render_widget(Paragraph::new(""), chunks[2]);
                if prompt.is_none() {
                    if let EditRow::Title | EditRow::Url | EditRow::Description | EditRow::Tags | EditRow::Folder =
                        EDIT_ROWS[focus]
                    {
                        let col = chunks[0].x + 12 + cursor.min(200) as u16;
                        f.set_cursor_position((col, chunks[0].y + 1 + focus as u16));
                    }
                } else if let Some((_, buf)) = &prompt {
                    let col = chunks[1].x + buf.len() as u16 + 2;
                    f.set_cursor_position((col, chunks[1].y));
                }
            })?;
            if !event::poll(std::time::Duration::from_millis(100))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('s') => {
                        return Ok(Some(EditDraft {
                            title: Some(fields[0].clone()),
                            description: Some(fields[2].clone()),
                            tags: Some(fields[3].split_whitespace().map(str::to_string).collect()),
                            folder: Some(fields[4].clone()),
                            url: Some(fields[1].clone()),
                            markdown: if markdown_remove {
                                MarkdownAction::Remove
                            } else if markdown_add {
                                MarkdownAction::Add
                            } else {
                                MarkdownAction::Keep
                            },
                            archive: if archive_remove {
                                ArchiveAction::Remove
                            } else if archive_add {
                                ArchiveAction::Add {
                                    backend: Some(BACKENDS[backend_idx].to_string()),
                                }
                            } else {
                                ArchiveAction::Keep
                            },
                            attach_paths: attach.clone(),
                            detach: detach.clone(),
                        }));
                    }
                    KeyCode::Char('c') => return Ok(None),
                    _ => {}
                }
                continue;
            }
            if let Some((kind, buf)) = prompt.take() {
                match key.code {
                    KeyCode::Esc => {
                        message.clear();
                    }
                    KeyCode::Enter => {
                        let value = buf.trim().to_string();
                        if !value.is_empty() {
                            if kind == "attach" {
                                attach.push(std::path::PathBuf::from(value));
                            } else {
                                detach.push(value);
                            }
                            message.clear();
                        }
                    }
                    KeyCode::Backspace => {
                        let mut buf = buf;
                        buf.pop();
                        prompt = Some((kind, buf));
                    }
                    KeyCode::Char(c) => {
                        let mut buf = buf;
                        buf.push(c);
                        prompt = Some((kind, buf));
                    }
                    _ => {
                        prompt = Some((kind, buf));
                    }
                }
                continue;
            }
            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Tab => {
                    focus = (focus + 1) % EDIT_ROWS.len();
                    cursor = usize::MAX;
                    message.clear();
                }
                KeyCode::BackTab => {
                    focus = (focus + EDIT_ROWS.len() - 1) % EDIT_ROWS.len();
                    cursor = usize::MAX;
                    message.clear();
                }
                KeyCode::Up => {
                    focus = (focus + EDIT_ROWS.len() - 1) % EDIT_ROWS.len();
                    cursor = usize::MAX;
                }
                KeyCode::Down => {
                    focus = (focus + 1) % EDIT_ROWS.len();
                    cursor = usize::MAX;
                }
                KeyCode::Left => {
                    cursor = cursor.saturating_sub(1);
                }
                KeyCode::Right => {
                    cursor = cursor.saturating_add(1);
                }
                KeyCode::Char(' ') => {
                    if let Some(idx) = field_index(EDIT_ROWS[focus]) {
                        insert_at(&mut fields[idx], &mut cursor, ' ');
                    } else {
                        match EDIT_ROWS[focus] {
                            EditRow::Markdown => {
                                if b.markdown_file.is_some() {
                                    markdown_remove = !markdown_remove;
                                } else {
                                    markdown_add = !markdown_add;
                                }
                            }
                            EditRow::Archive => {
                                if b.archive_file.is_some() {
                                    archive_remove = !archive_remove;
                                } else {
                                    archive_add = !archive_add;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                KeyCode::Enter => match EDIT_ROWS[focus] {
                    EditRow::Markdown => {
                        if b.markdown_file.is_some() {
                            markdown_remove = !markdown_remove;
                        } else {
                            markdown_add = !markdown_add;
                        }
                    }
                    EditRow::Archive => {
                        if b.archive_file.is_some() {
                            archive_remove = !archive_remove;
                        } else {
                            archive_add = !archive_add;
                        }
                    }
                    EditRow::Backend => {
                        backend_idx = (backend_idx + 1) % BACKENDS.len();
                    }
                    _ => {}
                },
                KeyCode::Backspace => {
                    if let Some(idx) = field_index(EDIT_ROWS[focus]) {
                        delete_at(&mut fields[idx], &mut cursor);
                    }
                }
                KeyCode::Delete => {
                    if let Some(idx) = field_index(EDIT_ROWS[focus]) {
                        delete_at(&mut fields[idx], &mut cursor);
                    }
                }
                KeyCode::Char('a') => {
                    if EDIT_ROWS[focus] == EditRow::Attach {
                        prompt = Some(("attach path".to_string(), String::new()));
                    } else if let Some(idx) = field_index(EDIT_ROWS[focus]) {
                        insert_at(&mut fields[idx], &mut cursor, 'a');
                    }
                }
                KeyCode::Char('d') => {
                    if EDIT_ROWS[focus] == EditRow::Detach {
                        prompt = Some(("detach name or #".to_string(), String::new()));
                    } else if let Some(idx) = field_index(EDIT_ROWS[focus]) {
                        insert_at(&mut fields[idx], &mut cursor, 'd');
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(idx) = field_index(EDIT_ROWS[focus]) {
                        insert_at(&mut fields[idx], &mut cursor, c);
                    }
                }
                _ => {}
            }
        }
    })
}

fn insert_at(s: &mut String, cursor: &mut usize, c: char) {
    if *cursor == usize::MAX {
        *cursor = s.len();
    }
    let pos = (*cursor).min(s.len());
    s.insert(pos, c);
    *cursor = pos + 1;
}

fn delete_at(s: &mut String, cursor: &mut usize) {
    if *cursor == usize::MAX {
        *cursor = s.len();
    }
    if *cursor == 0 || s.is_empty() {
        return;
    }
    let pos = (*cursor).min(s.len());
    s.remove(pos - 1);
    *cursor = pos - 1;
}
