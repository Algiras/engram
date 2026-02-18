use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::{App, TuiAction};
use crate::tui::data::MemoryItem;

/// Render the screen tab bar at the top, highlighting the active screen.
fn render_screen_tabs(f: &mut Frame, active: &str, area: Rect) {
    let tabs = [
        ("Browser", "B"),
        ("Packs", "p"),
        ("Learning", "L"),
        ("Analytics", "A"),
        ("Health", "H"),
        ("Daemon", "D"),
        ("Config", "C"),
        ("Help", "?"),
    ];

    let mut spans = Vec::new();
    spans.push(Span::raw(" "));

    for (i, (name, key)) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
        if *name == active {
            spans.push(Span::styled(
                format!(" {} ", name),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("[{}]", key),
                Style::default().fg(Color::Cyan),
            ));
            spans.push(Span::styled(
                format!(" {} ", name),
                Style::default().fg(Color::Gray),
            ));
        }
    }

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));
    f.render_widget(bar, area);
}

/// Render the two-panel browser screen.
pub fn render_browser(f: &mut Frame, app: &App) {
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(f.area());

    // Top: screen tabs
    render_screen_tabs(f, "Browser", layout[0]);

    // Middle: two-panel browser
    let panels = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(layout[1]);

    render_project_list(f, app, panels[0]);
    render_item_list(f, app, panels[1]);

    // Overlay delete dialog if active
    if app.show_delete {
        render_delete_dialog(f, app);
    }

    // Status bar at bottom
    render_status_bar(f, app, layout[2]);

    // Global overlays (action confirm / action message)
    render_action_overlays(f, app);
}

fn render_project_list(f: &mut Frame, app: &App, area: Rect) {
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

fn render_status_bar(f: &mut Frame, app: &App, bar_area: Rect) {
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

    let status = if app.show_delete {
        Line::from(Span::styled(
            " y: confirm delete | n/Esc: cancel ",
            Style::default().fg(Color::Black),
        ))
    } else if !app.search_matches.is_empty() {
        Line::from(Span::styled(
            " j/k: nav | /: search | n/N: match | Enter: view | d: del | q: quit ",
            Style::default().fg(Color::Black),
        ))
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("j/k", Style::default().fg(Color::Cyan)),
            Span::raw(": nav  "),
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(": search  "),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::raw(": view  "),
            Span::styled("d", Style::default().fg(Color::Cyan)),
            Span::raw(": del  │  "),
            Span::styled("i", Style::default().fg(Color::Yellow)),
            Span::raw(": ingest  "),
            Span::styled("R", Style::default().fg(Color::Yellow)),
            Span::raw(": regen  "),
            Span::styled("I", Style::default().fg(Color::Yellow)),
            Span::raw(": inject  │  "),
            Span::styled("?", Style::default().fg(Color::Cyan)),
            Span::raw(": help  "),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::raw(": quit"),
        ])
    };

    let bar = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));

    f.render_widget(bar, bar_area);
}

/// Render the full-screen markdown viewer.
pub fn render_viewer(f: &mut Frame, app: &App) {
    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Browser", layout[0]);

    let block = Block::default()
        .title(" Viewer (Esc: back, PgUp/PgDn: scroll) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(app.viewer_content.as_str())
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    f.render_widget(paragraph, layout[1]);

    // Status bar
    let help = format!(
        " Esc/q: back | PgUp/PgDn/j/k: scroll | Line {} ",
        app.scroll_offset + 1
    );
    let bar = Paragraph::new(help).style(Style::default().fg(Color::Black).bg(Color::DarkGray));
    f.render_widget(bar, layout[2]);
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
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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

    // Split into tab bar, main area, and status bar
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Packs", chunks[0]);
    let chunks = [chunks[1], chunks[2]];

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
                "  engram hive browse",
                Style::default().fg(Color::Green),
            )),
            Line::from("  engram hive install <pack-name>"),
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
                let _keywords = if pack.keywords.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", pack.keywords.join(", "))
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(
                            format!("● {}", pack.name),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
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

        let list = List::new(items).block(title_block).highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(list, chunks[0], &mut list_state);
    }

    // Status bar
    let status_line = if app.pack_search_mode {
        Line::from(format!(
            "SEARCH: {}  (Enter: jump, ESC: cancel)",
            app.pack_search_query
        ))
    } else if !app.pack_search_matches.is_empty() {
        Line::from(format!(
            "j/k: nav  |  Enter: details  |  u: update  |  d: del  |  /: search  |  n/N: match {}/{}  |  ESC: back  |  q: quit",
            app.pack_search_index + 1,
            app.pack_search_matches.len()
        ))
    } else if app.packs.is_empty() {
        Line::from("ESC: back to browser  |  q: quit")
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("j/k", Style::default().fg(Color::Cyan)),
            Span::raw(": nav  "),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::raw(": details  "),
            Span::styled("u", Style::default().fg(Color::Cyan)),
            Span::raw(": update  "),
            Span::styled("d", Style::default().fg(Color::Cyan)),
            Span::raw(": del  │  "),
            Span::styled("g", Style::default().fg(Color::Yellow)),
            Span::raw(": graph  │  "),
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(": search  "),
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::raw(": reload  "),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::raw(": quit"),
        ])
    };

    let status_bar =
        Paragraph::new(status_line).style(Style::default().bg(Color::DarkGray).fg(Color::White));

    f.render_widget(status_bar, chunks[1]);

    // Overlays
    if app.show_pack_confirm.is_some() {
        render_pack_confirm_dialog(f, app);
    }

    if app.pack_action_message.is_some() {
        render_pack_action_message(f, app);
    }

    render_action_overlays(f, app);
}

/// Render pack detail screen
pub fn render_pack_detail(f: &mut Frame, app: &App) {
    let area = f.area();

    // Split into tab bar, content area, and status bar
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Packs", chunks[0]);
    let chunks = [chunks[1], chunks[2]];

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
        let pack_name = app
            .packs
            .get(app.pack_index)
            .map(|p| p.name.as_str())
            .unwrap_or("unknown");

        let (title, message, warning) = match action {
            super::PackAction::Update => (
                "Update Pack",
                format!("Update pack '{}'?", pack_name),
                "This will pull the latest version from the registry.",
            ),
            super::PackAction::Uninstall => (
                "Uninstall Pack",
                format!("Uninstall pack '{}'?", pack_name),
                "This will remove the pack and all its knowledge from your system.",
            ),
        };

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                title,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
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

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

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
            Line::from(Span::styled(
                "Press any key to continue",
                Style::default().fg(Color::Gray),
            )),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}

/// Render Learning Dashboard screen
pub fn render_learning(f: &mut Frame, app: &App) {
    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Learning", layout[0]);
    let main_area = layout[1];

    // Scroll the content
    let lines: Vec<Line> = app
        .learning_content
        .lines()
        .skip(app.learning_scroll as usize)
        .take(main_area.height.saturating_sub(2) as usize)
        .map(|line| Line::from(line.to_string()))
        .collect();

    let title = if let Some(project) = app.tree.projects.get(app.project_index) {
        format!(" Learning Dashboard: {} ", project.name)
    } else {
        " Learning Dashboard ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, main_area);

    // Status bar
    let status = Line::from(vec![
        Span::raw(" ["),
        Span::styled("q/Esc", Style::default().fg(Color::Cyan)),
        Span::raw("] Back  ["),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw("] Reload  ["),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw("] Simulate  ["),
        Span::styled("o", Style::default().fg(Color::Yellow)),
        Span::raw("] Optimize  ["),
        Span::styled("j/k", Style::default().fg(Color::Cyan)),
        Span::raw("] Scroll"),
    ]);

    f.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray)),
        layout[2],
    );

    render_action_overlays(f, app);
}

/// Render Analytics Viewer screen
pub fn render_analytics(f: &mut Frame, app: &App) {
    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Analytics", layout[0]);
    let main_area = layout[1];

    // Scroll the content
    let lines: Vec<Line> = app
        .analytics_content
        .lines()
        .skip(app.analytics_scroll as usize)
        .take(main_area.height.saturating_sub(2) as usize)
        .map(|line| Line::from(line.to_string()))
        .collect();

    let title = if let Some(project) = app.tree.projects.get(app.project_index) {
        format!(
            " Analytics: {} ({} days) ",
            project.name, app.analytics_days
        )
    } else {
        format!(" Analytics ({} days) ", app.analytics_days)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, main_area);

    // Status bar
    let status = Line::from(vec![
        Span::raw(" ["),
        Span::styled("q/Esc", Style::default().fg(Color::Cyan)),
        Span::raw("] Back  ["),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw("] Reload  ["),
        Span::styled("+/-", Style::default().fg(Color::Cyan)),
        Span::raw("] Days  ["),
        Span::styled("j/k", Style::default().fg(Color::Cyan)),
        Span::raw("] Scroll"),
    ]);

    f.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray)),
        layout[2],
    );
}

/// Render Health Check screen
pub fn render_health(f: &mut Frame, app: &App) {
    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Health", layout[0]);
    let main_area = layout[1];

    // Scroll the content
    let lines: Vec<Line> = app
        .health_content
        .lines()
        .skip(app.health_scroll as usize)
        .take(main_area.height.saturating_sub(2) as usize)
        .map(|line| Line::from(line.to_string()))
        .collect();

    let title = if let Some(project) = app.tree.projects.get(app.project_index) {
        format!(" Health Check: {} ", project.name)
    } else {
        " Health Check ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, main_area);

    // Status bar
    let status = Line::from(vec![
        Span::raw(" ["),
        Span::styled("q/Esc", Style::default().fg(Color::Cyan)),
        Span::raw("] Back  ["),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw("] Reload  ["),
        Span::styled("x", Style::default().fg(Color::Yellow)),
        Span::raw("] Doctor  ["),
        Span::styled("c", Style::default().fg(Color::Yellow)),
        Span::raw("] Cleanup  ["),
        Span::styled("j/k", Style::default().fg(Color::Cyan)),
        Span::raw("] Scroll"),
    ]);

    f.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray)),
        layout[2],
    );

    render_action_overlays(f, app);
}

/// Render Daemon screen
pub fn render_daemon(f: &mut Frame, app: &App) {
    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Daemon", layout[0]);
    let main_area = layout[1];

    let lines: Vec<Line> = app
        .daemon_content
        .lines()
        .skip(app.daemon_scroll as usize)
        .take(main_area.height.saturating_sub(2) as usize)
        .map(|line| {
            if line.contains("RUNNING") {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Green),
                ))
            } else if line.contains("STOPPED") {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Yellow),
                ))
            } else {
                Line::from(line.to_string())
            }
        })
        .collect();

    let title = format!(
        " Daemon — interval: {}min (use +/- to adjust) ",
        app.daemon_interval
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, main_area);

    // Status bar
    let status = Line::from(vec![
        Span::raw(" ["),
        Span::styled("q/Esc", Style::default().fg(Color::Cyan)),
        Span::raw("] Back  ["),
        Span::styled("r", Style::default().fg(Color::Cyan)),
        Span::raw("] Reload  ["),
        Span::styled("s", Style::default().fg(Color::Green)),
        Span::raw("] Start  ["),
        Span::styled("x", Style::default().fg(Color::Red)),
        Span::raw("] Stop  ["),
        Span::styled("+/-", Style::default().fg(Color::Yellow)),
        Span::raw("] Interval  ["),
        Span::styled("j/k", Style::default().fg(Color::Cyan)),
        Span::raw("] Scroll"),
    ]);

    f.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray)),
        layout[2],
    );

    render_action_overlays(f, app);
}

/// Render Help screen
pub fn render_help(f: &mut Frame, _app: &App) {
    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Help", layout[0]);
    let main_area = layout[1];

    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Navigation:"),
        Line::from("  j/k, ↓/↑      - Move cursor up/down"),
        Line::from("  h/l, ←/→, Tab - Switch panels"),
        Line::from("  Enter         - View selected item"),
        Line::from("  q, Ctrl+C     - Quit"),
        Line::from("  Esc           - Go back"),
        Line::from(""),
        Line::from("Browser Screen:"),
        Line::from("  /             - Search"),
        Line::from("  n/N           - Next/previous search match"),
        Line::from("  d             - Delete item"),
        Line::from("  i             - Ingest knowledge from conversations"),
        Line::from("  R             - Regenerate context for project"),
        Line::from("  I             - Inject memory into Claude Code"),
        Line::from(""),
        Line::from("Packs Screen:"),
        Line::from("  Enter         - View pack details"),
        Line::from("  u             - Update pack"),
        Line::from("  d             - Uninstall pack"),
        Line::from("  g             - Build knowledge graph"),
        Line::from("  r             - Reload packs"),
        Line::from("  /             - Search packs"),
        Line::from(""),
        Line::from("Learning Screen:"),
        Line::from("  s             - Run learning simulation"),
        Line::from("  o             - Apply learned optimizations"),
        Line::from("  r             - Reload data"),
        Line::from(""),
        Line::from("Health Screen:"),
        Line::from("  x             - Run doctor (health check + auto-fix)"),
        Line::from("  c             - Cleanup expired entries"),
        Line::from("  r             - Reload data"),
        Line::from(""),
        Line::from("Daemon Screen:"),
        Line::from("  s             - Start daemon (uses current interval)"),
        Line::from("  x             - Stop daemon"),
        Line::from("  +/-           - Adjust polling interval (minutes)"),
        Line::from("  r             - Reload status & logs"),
        Line::from(""),
        Line::from("Config Screen (C):"),
        Line::from("  Tab           - Switch between LLM panel / Embed panel"),
        Line::from("  j/k           - Navigate providers in focused panel"),
        Line::from("  Enter         - Set default LLM  /  select embed provider"),
        Line::from("  T             - Test selected provider (live API ping)"),
        Line::from("  M             - Set model:"),
        Line::from("                  OpenAI/Ollama/VSCode/OpenRouter → scrollable picker"),
        Line::from("                  Anthropic/Gemini → text input"),
        Line::from("  q/Esc         - Back to Browser"),
        Line::from(""),
        Line::from("Providers (engram auth list/status):"),
        Line::from("  anthropic     - Claude models, requires ANTHROPIC_API_KEY"),
        Line::from("  openai        - GPT models, requires OPENAI_API_KEY"),
        Line::from("  gemini        - Google models, requires GEMINI_API_KEY"),
        Line::from("  openrouter    - 100+ models, many free ★, requires OPENROUTER_API_KEY"),
        Line::from("  vscode        - VS Code LM bridge, no key needed"),
        Line::from("  ollama        - Local models, no key needed"),
        Line::from(""),
        Line::from("Viewer/Detail Screens:"),
        Line::from("  j/k           - Scroll line by line"),
        Line::from("  Space/PgDn    - Scroll page down"),
        Line::from("  PgUp          - Scroll page up"),
        Line::from("  g/Home        - Go to top"),
        Line::from("  G/End         - Go to bottom"),
        Line::from(""),
        Line::from("Analytics Screen:"),
        Line::from("  +/-           - Increase/decrease days"),
        Line::from(""),
        Line::from(Span::styled(
            format!("engram v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::Gray),
        )),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, main_area);

    // Status bar
    let status = Line::from(vec![
        Span::raw(" ["),
        Span::styled("q/Esc/?", Style::default().fg(Color::Cyan)),
        Span::raw("] Close"),
    ]);

    f.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray)),
        layout[2],
    );
}

/// Render Config screen — provider and embedding selection with live connectivity testing.
pub fn render_config(f: &mut Frame, app: &App) {
    use crate::auth::{providers::Provider, AuthStore};

    let area = f.area();

    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    render_screen_tabs(f, "Config", layout[0]);

    // Split content into LLM (top 60%) and Embed (bottom 40%)
    let content_chunks =
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(layout[1]);

    // --- LLM Providers list ---
    let store = AuthStore::load().unwrap_or_default();
    let llm_providers = Provider::all();

    let llm_items: Vec<ListItem> = llm_providers
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let model = store
                .get(p)
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| p.default_model().to_string());
            let is_default = store.default_provider.as_deref() == Some(&p.to_string());
            let default_tag = if is_default { "  [default]" } else { "" };
            let line = format!(
                "{}  {:<28}  model: {:<30}{}",
                if is_default { "✓" } else { "○" },
                p.display_name(),
                model,
                default_tag
            );
            let style = if i == app.config_llm_index && app.config_focus_llm {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if i == app.config_llm_index {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let llm_border_style = if app.config_focus_llm {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let llm_block = Block::default()
        .title(" LLM Providers  [Enter] set default  [T] test  [M] set model ")
        .borders(Borders::ALL)
        .border_style(llm_border_style);

    let mut llm_state = ListState::default();
    llm_state.select(Some(app.config_llm_index));
    f.render_stateful_widget(
        List::new(llm_items).block(llm_block),
        content_chunks[0],
        &mut llm_state,
    );

    // --- Embedding Providers list ---
    let embed_options = [
        ("openai", "OpenAI text-embedding-3-small"),
        ("gemini", "Gemini text-embedding-004"),
        ("ollama", "Ollama nomic-embed-text"),
    ];

    let embed_items: Vec<ListItem> = embed_options
        .iter()
        .enumerate()
        .map(|(i, (key, label))| {
            let is_selected = store.embed_provider.as_deref() == Some(key);
            let is_inferred = store.embed_provider.is_none() && i == 0;
            let tag = if is_selected {
                "  [selected]"
            } else if is_inferred {
                "  (inferred)"
            } else {
                ""
            };
            let line = format!("{}  {}{}", if is_selected { "●" } else { "○" }, label, tag);
            let style = if i == app.config_embed_index && !app.config_focus_llm {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if i == app.config_embed_index {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let embed_border_style = if !app.config_focus_llm {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let embed_block = Block::default()
        .title(" Embedding Provider  [Enter] select ")
        .borders(Borders::ALL)
        .border_style(embed_border_style);

    let mut embed_state = ListState::default();
    embed_state.select(Some(app.config_embed_index));
    f.render_stateful_widget(
        List::new(embed_items).block(embed_block),
        content_chunks[1],
        &mut embed_state,
    );

    // --- Status bar ---
    let status_style =
        if app.config_status.starts_with("OK") || app.config_status.starts_with("Set") {
            Style::default().fg(Color::Green).bg(Color::DarkGray)
        } else if app.config_status.starts_with("FAIL") || app.config_status.starts_with("Error") {
            Style::default().fg(Color::Red).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        };

    let status_text = if app.config_test_running {
        " Testing... (waiting for API response) ".to_string()
    } else {
        format!(" {} ", app.config_status)
    };

    f.render_widget(Paragraph::new(status_text).style(status_style), layout[2]);

    // Model list picker overlay (for providers with /v1/models)
    if app.config_model_list_mode {
        render_model_list_overlay(f, app);
    } else if app.config_model_input_mode {
        // Text input fallback (Anthropic, Gemini)
        render_model_input_overlay(f, app);
    }
}

fn render_model_list_overlay(f: &mut Frame, app: &App) {
    let area = f.area();

    let popup_width = 72u16.min(area.width.saturating_sub(4));
    // Show up to 20 models + 4 chrome lines
    let visible = 20usize.min(area.height.saturating_sub(6) as usize);
    let popup_height = (visible as u16 + 4).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let provider = app.current_config_provider();
    let title = format!(
        " {} models ({} total) — ★ free ",
        provider.display_name(),
        app.config_model_list.len()
    );

    let items: Vec<ListItem> = app
        .config_model_list
        .iter()
        .skip(app.config_model_list_scroll)
        .take(visible)
        .enumerate()
        .map(|(i, model)| {
            let abs_index = i + app.config_model_list_scroll;
            let is_free = model.ends_with(":free");
            let prefix = if is_free { "★ " } else { "  " };
            let label = format!("{}{}", prefix, model);

            let style = if abs_index == app.config_model_list_index {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if is_free {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            ListItem::new(label).style(style)
        })
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut state = ListState::default();
    // The list only shows `visible` items starting from scroll, so the selected
    // item within the visible window is the cursor minus the scroll offset
    let visible_selected = app
        .config_model_list_index
        .saturating_sub(app.config_model_list_scroll);
    state.select(Some(visible_selected));

    f.render_widget(Clear, popup_area);

    // Split into list area + status line
    let inner = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(popup_area);

    f.render_stateful_widget(List::new(items).block(block), inner[0], &mut state);

    let help = Paragraph::new(" j/k: navigate  PgUp/PgDn: page  Enter: select  Esc: cancel ")
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(help, inner[1]);
}

fn render_model_input_overlay(f: &mut Frame, app: &App) {
    let area = f.area();

    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 6u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let text = vec![
        Line::from(""),
        Line::from("Enter model name:"),
        Line::from(""),
        Line::from(format!("> {}_", app.config_model_input)),
        Line::from(""),
        Line::from(Span::styled(
            "[Enter] confirm  [Esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" Set Model ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// Render action confirmation dialog and action result message overlays.
fn render_action_overlays(f: &mut Frame, app: &App) {
    if let Some(action) = &app.show_action_confirm {
        render_action_confirm_dialog(f, app, action);
    }
    if let Some((message, is_error)) = &app.action_message {
        render_action_message(f, message, *is_error);
    }
}

/// Render a confirmation dialog before running an action.
fn render_action_confirm_dialog(f: &mut Frame, app: &App, action: &TuiAction) {
    let area = f.area();
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 10u16.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let project_name = app
        .tree
        .projects
        .get(app.project_index)
        .map(|p| p.name.as_str())
        .unwrap_or("(all)");

    let (title, description, note) = match action {
        TuiAction::Ingest => (
            "Ingest Knowledge",
            format!(
                "Extract knowledge from conversations for '{}'?",
                project_name
            ),
            "This will parse recent conversations and extract knowledge.",
        ),
        TuiAction::Regen => (
            "Regenerate Context",
            format!("Regenerate context.md for '{}'?", project_name),
            "This will re-synthesize the project context from knowledge files.",
        ),
        TuiAction::Inject => (
            "Inject Memory",
            format!("Inject memory into Claude Code for '{}'?", project_name),
            "This writes MEMORY.md to the project's .claude/ directory.",
        ),
        TuiAction::LearnSimulate => (
            "Learning Simulation",
            format!("Run learning simulation for '{}'?", project_name),
            "This simulates recall patterns to train the learning system.",
        ),
        TuiAction::LearnOptimize => (
            "Apply Optimizations",
            format!("Apply learned optimizations for '{}'?", project_name),
            "This applies retention and importance adjustments from learning.",
        ),
        TuiAction::Doctor => (
            "Doctor (Health Check)",
            format!("Run doctor with auto-fix for '{}'?", project_name),
            "This checks for issues and automatically fixes what it can.",
        ),
        TuiAction::CleanupExpired => (
            "Cleanup Expired Entries",
            format!("Remove expired TTL entries for '{}'?", project_name),
            "This permanently removes entries whose TTL has elapsed.",
        ),
        TuiAction::GraphBuild => (
            "Build Knowledge Graph",
            format!("Build knowledge graph for '{}'?", project_name),
            "This extracts concepts and relationships from knowledge files.",
        ),
        TuiAction::DaemonStart => (
            "Start Daemon",
            format!(
                "Start background ingest daemon (interval: {}min)?",
                app.daemon_interval
            ),
            "Daemon will auto-ingest new sessions in the background.",
        ),
        TuiAction::DaemonStop => (
            "Stop Daemon",
            "Stop the running background daemon?".to_string(),
            "The daemon process will be terminated.",
        ),
    };

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(description),
        Line::from(""),
        Line::from(Span::styled(note, Style::default().fg(Color::Gray))),
        Line::from(""),
        Line::from(Span::styled(
            "Proceed? (y/n)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// Render an action result message popup.
fn render_action_message(f: &mut Frame, message: &str, is_error: bool) {
    let area = f.area();
    let popup_width = ((message.len() as u16) + 10)
        .min(area.width.saturating_sub(4))
        .max(40);
    let popup_height = 5u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let color = if is_error { Color::Red } else { Color::Green };

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            message,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to continue",
            Style::default().fg(Color::Gray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color));

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}
