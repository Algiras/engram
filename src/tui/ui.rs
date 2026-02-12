use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::App;
use crate::tui::data::MemoryItem;

/// Render the two-panel browser screen.
pub fn render_browser(f: &mut Frame, app: &App) {
    let chunks = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.area());

    render_project_list(f, app, chunks[0]);
    render_item_list(f, app, chunks[1]);

    // Overlay delete dialog if active
    if app.show_delete {
        render_delete_dialog(f, app);
    }

    // Status bar at bottom
    render_status_bar(f, app);
}

fn render_project_list(f: &mut Frame, app: &App, area: Rect) {
    // Reserve 1 row at bottom for status bar
    let area = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };

    let items: Vec<ListItem> = app
        .tree
        .projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == app.project_index && app.focus_left {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if i == app.project_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if app.is_project_search_match(i) {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            let session_count = p
                .items
                .iter()
                .filter(|it| matches!(it, MemoryItem::Session { .. }))
                .count();
            let knowledge_count = p
                .items
                .iter()
                .filter(|it| matches!(it, MemoryItem::KnowledgeFile { .. }))
                .count();
            let label = format!("{} ({}/{})", p.name, session_count, knowledge_count);
            ListItem::new(label).style(style)
        })
        .collect();

    let title = format!(" Projects ({}) ", app.tree.projects.len());
    let border_style = if app.focus_left {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let list = List::new(items).block(block);

    let mut state = ListState::default();
    state.select(Some(app.project_index));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_item_list(f: &mut Frame, app: &App, area: Rect) {
    let area = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };

    let project = app.tree.projects.get(app.project_index);
    let items: Vec<ListItem> = if let Some(proj) = project {
        proj.items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_match = app.is_search_match(app.project_index, i);
                let style = if i == app.item_index && !app.focus_left {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if i == app.item_index {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if is_match {
                    Style::default().fg(Color::Yellow)
                } else {
                    match item {
                        MemoryItem::Session { .. } => Style::default().fg(Color::White),
                        MemoryItem::KnowledgeFile { .. } => Style::default().fg(Color::Green),
                    }
                };
                let prefix = match item {
                    MemoryItem::Session { .. } => "📁",
                    MemoryItem::KnowledgeFile { .. } => "📄",
                };
                ListItem::new(format!("{} {}", prefix, item.display_label())).style(style)
            })
            .collect()
    } else {
        vec![]
    };

    let title = project
        .map(|p| format!(" {} ", p.name))
        .unwrap_or_else(|| " (no project) ".to_string());
    let border_style = if !app.focus_left {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let list = List::new(items).block(block);

    let mut state = ListState::default();
    state.select(Some(app.item_index));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_status_bar(f: &mut Frame, app: &App) {
    let area = f.area();
    let bar_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };

    if app.search_mode {
        let match_count = app.search_matches.len();
        let text = format!(" Search: {}_ ({} matches)", app.search_query, match_count);
        let bar = Paragraph::new(Line::from(vec![Span::styled(
            text,
            Style::default().fg(Color::Yellow).bg(Color::DarkGray),
        )]))
        .style(Style::default().bg(Color::DarkGray));
        f.render_widget(bar, bar_area);
        return;
    }

    let help = if app.show_delete {
        " y: confirm delete | n/Esc: cancel "
    } else if !app.search_matches.is_empty() {
        " j/k: navigate | /: search | n/N: next/prev match | Enter: view | d: delete | q: quit "
    } else {
        " j/k: navigate | /: search | Tab/h/l: switch panel | Enter: view | d: delete | q: quit "
    };

    let bar = Paragraph::new(Line::from(vec![Span::styled(
        help,
        Style::default().fg(Color::Black).bg(Color::DarkGray),
    )]))
    .style(Style::default().bg(Color::DarkGray));

    f.render_widget(bar, bar_area);
}

/// Render the full-screen markdown viewer.
pub fn render_viewer(f: &mut Frame, app: &App) {
    let area = f.area();

    // Main content area (reserve 1 row for status)
    let content_area = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };

    let block = Block::default()
        .title(" Viewer (Esc: back, PgUp/PgDn: scroll) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(app.viewer_content.as_str())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    f.render_widget(paragraph, content_area);

    // Status bar
    let bar_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    let help = format!(
        " Esc/q: back | PgUp/PgDn/j/k: scroll | Line {} ",
        app.scroll_offset + 1
    );
    let bar = Paragraph::new(help)
        .style(Style::default().fg(Color::Black).bg(Color::DarkGray));
    f.render_widget(bar, bar_area);
}

/// Render a centered delete confirmation dialog over the browser.
fn render_delete_dialog(f: &mut Frame, app: &App) {
    let area = f.area();

    // Centered popup: 50 wide, 8 tall (2 borders + 6 content lines)
    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 8u16.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Get the item name being deleted
    let item_name = app
        .current_item()
        .map(|item| match item {
            MemoryItem::Session { session_id, .. } => format!("session: {}", session_id),
            MemoryItem::KnowledgeFile { name, .. } => format!("file: {}", name),
        })
        .unwrap_or_else(|| "unknown".to_string());

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Delete this item?",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(item_name),
        Line::from(""),
        Line::from(Span::styled(
            "y: yes  |  n/Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let dialog = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(Clear, popup_area);
    f.render_widget(dialog, popup_area);
}

/// Render the packs browser screen
pub fn render_packs(f: &mut Frame, app: &App) {
    let area = f.area();

    // Split into main area and status bar
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
    ]).split(area);

    // Title
    let title_block = Block::default()
        .title(" Installed Knowledge Packs ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    if app.packs.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "No packs installed",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from("Browse and install packs with:"),
            Line::from(Span::styled(
                "  claude-memory hive browse",
                Style::default().fg(Color::Green),
            )),
            Line::from("  claude-memory hive install <pack-name>"),
        ];

        let paragraph = Paragraph::new(empty_text)
            .block(title_block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, chunks[0]);
    } else {
        // Render pack list
        let items: Vec<ListItem> = app
            .packs
            .iter()
            .enumerate()
            .map(|(i, pack)| {
                let style = if i == app.pack_index {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let categories = pack.categories.join(", ");
                let keywords = if pack.keywords.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", pack.keywords.join(", "))
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(
                            format!("● {}", pack.name),
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!(" v{}", pack.version)),
                    ]),
                    Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::raw(&pack.description),
                    ]),
                    Line::from(vec![
                        Span::styled("  Categories: ", Style::default().fg(Color::Gray)),
                        Span::raw(categories),
                    ]),
                    Line::from(vec![
                        Span::styled("  Registry: ", Style::default().fg(Color::Gray)),
                        Span::raw(&pack.registry),
                        Span::styled("  Installed: ", Style::default().fg(Color::Gray)),
                        Span::raw(pack.installed_at.format("%Y-%m-%d").to_string()),
                    ]),
                ];

                ListItem::new(content).style(style)
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(app.pack_index));

        let list = List::new(items)
            .block(title_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_stateful_widget(list, chunks[0], &mut list_state);
    }

    // Status bar
    let status_text = if app.pack_search_mode {
        format!("SEARCH: {}  (Enter: jump, ESC: cancel)", app.pack_search_query)
    } else if !app.pack_search_matches.is_empty() {
        format!(
            "j/k: nav  |  Enter: details  |  u: update  |  d: del  |  /: search  |  n/N: match {}/{}  |  ESC: back  |  q: quit",
            app.pack_search_index + 1,
            app.pack_search_matches.len()
        )
    } else if app.packs.is_empty() {
        "ESC: back to browser  |  q: quit".to_string()
    } else {
        "j/k: nav  |  Enter: details  |  u: update  |  d: del  |  /: search  |  r: reload  |  ESC: back  |  q: quit".to_string()
    };

    let status_bar = Paragraph::new(status_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .alignment(Alignment::Center);

    f.render_widget(status_bar, chunks[1]);

    // Overlays
    if app.show_pack_confirm.is_some() {
        render_pack_confirm_dialog(f, app);
    }

    if app.pack_action_message.is_some() {
        render_pack_action_message(f, app);
    }
}

/// Render pack detail screen
pub fn render_pack_detail(f: &mut Frame, app: &App) {
    let area = f.area();

    // Split into content area and status bar
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
    ]).split(area);

    let block = Block::default()
        .title(" Pack Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(app.pack_detail_content.as_str())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.pack_detail_scroll, 0));

    f.render_widget(paragraph, chunks[0]);

    // Status bar
    let help = format!(
        " Esc: back to packs | PgUp/PgDn/j/k: scroll | Line {} ",
        app.pack_detail_scroll + 1
    );
    let bar = Paragraph::new(help)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(bar, chunks[1]);
}

/// Render pack action confirmation dialog
fn render_pack_confirm_dialog(f: &mut Frame, app: &App) {
    let area = f.area();

    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 10u16.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    if let Some(action) = &app.show_pack_confirm {
        let pack_name = app.packs.get(app.pack_index)
            .map(|p| p.name.as_str())
            .unwrap_or("unknown");

        let (title, message, warning) = match action {
            super::PackAction::Update => (
                "Update Pack",
                format!("Update pack '{}'?", pack_name),
                "This will pull the latest version from the registry."
            ),
            super::PackAction::Uninstall => (
                "Uninstall Pack",
                format!("Uninstall pack '{}'?", pack_name),
                "This will remove the pack and all its knowledge from your system."
            ),
        };

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                title,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(message),
            Line::from(""),
            Line::from(Span::styled(warning, Style::default().fg(Color::Gray))),
            Line::from(""),
            Line::from(Span::styled(
                "Continue? (y/n)",
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(text).block(block).alignment(Alignment::Center);

        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}

/// Render pack action message (success/error)
fn render_pack_action_message(f: &mut Frame, app: &App) {
    if let Some((message, is_error)) = &app.pack_action_message {
        let area = f.area();
        
        let popup_width = (message.len() as u16 + 10).min(area.width.saturating_sub(4));
        let popup_height = 5u16;
        let x = (area.width.saturating_sub(popup_width)) / 2;
        let y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        let color = if *is_error { Color::Red } else { Color::Green };

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                message,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled("Press any key to continue", Style::default().fg(Color::Gray))),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color));

        let paragraph = Paragraph::new(text).block(block).alignment(Alignment::Center);

        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}
