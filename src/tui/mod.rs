pub mod data;
mod ui;

use std::io;
use std::path::PathBuf;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use data::{MemoryItem, MemoryTree};

enum Screen {
    Browser,
    Viewer,
    Packs,
    PackDetail,
}

#[derive(Clone, PartialEq)]
pub enum PackAction {
    Uninstall,
    Update,
}

pub struct App {
    screen: Screen,
    tree: MemoryTree,
    project_index: usize,
    item_index: usize,
    focus_left: bool,
    scroll_offset: u16,
    viewer_content: String,
    show_delete: bool,
    memory_dir: PathBuf,
    // Search state
    search_mode: bool,
    search_query: String,
    search_matches: Vec<(usize, usize, i64)>, // (project_idx, item_idx, score)
    search_match_index: usize,
    fuzzy_matcher: SkimMatcherV2,
    // Packs state
    packs: Vec<data::PackEntry>,
    pack_index: usize,
    pack_detail_content: String,
    pack_detail_scroll: u16,
    pub pack_action_message: Option<(String, bool)>, // (message, is_error)
    pub show_pack_confirm: Option<PackAction>,
    // Pack search state
    pack_search_mode: bool,
    pack_search_query: String,
    pack_search_matches: Vec<usize>, // Indices of matching packs
    pack_search_index: usize,
}

impl App {
    pub fn new(memory_dir: PathBuf) -> App {
        let tree = data::load_tree(&memory_dir);
        let packs = data::load_packs(&memory_dir);
        App {
            screen: Screen::Browser,
            tree,
            project_index: 0,
            item_index: 0,
            focus_left: true,
            scroll_offset: 0,
            viewer_content: String::new(),
            show_delete: false,
            memory_dir: memory_dir.clone(),
            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_index: 0,
            fuzzy_matcher: SkimMatcherV2::default(),
            packs,
            pack_index: 0,
            pack_detail_content: String::new(),
            pack_detail_scroll: 0,
            pack_action_message: None,
            show_pack_confirm: None,
            pack_search_mode: false,
            pack_search_query: String::new(),
            pack_search_matches: Vec::new(),
            pack_search_index: 0,
        }
    }

    fn current_item(&self) -> Option<&MemoryItem> {
        self.tree
            .projects
            .get(self.project_index)
            .and_then(|p| p.items.get(self.item_index))
    }

    fn project_item_count(&self) -> usize {
        self.tree
            .projects
            .get(self.project_index)
            .map(|p| p.items.len())
            .unwrap_or(0)
    }

    fn reload_tree(&mut self) {
        self.tree = data::load_tree(&self.memory_dir);
        // Clamp indices
        if self.project_index >= self.tree.projects.len() && !self.tree.projects.is_empty() {
            self.project_index = self.tree.projects.len() - 1;
        }
        let count = self.project_item_count();
        if self.item_index >= count && count > 0 {
            self.item_index = count - 1;
        }
    }

    fn compute_search_matches(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }

        for (pi, project) in self.tree.projects.iter().enumerate() {
            // Fuzzy match on project name
            if let Some(project_score) = self.fuzzy_matcher.fuzzy_match(&project.name, &self.search_query) {
                // Add all items from matching project with project score
                if project.items.is_empty() {
                    self.search_matches.push((pi, 0, project_score));
                } else {
                    for (ii, _) in project.items.iter().enumerate() {
                        self.search_matches.push((pi, ii, project_score));
                    }
                }
            }

            // Fuzzy match on item labels
            for (ii, item) in project.items.iter().enumerate() {
                if let Some(item_score) = self.fuzzy_matcher.fuzzy_match(&item.display_label(), &self.search_query) {
                    // Only add if not already added from project match
                    if !self.search_matches.iter().any(|(p, i, _)| *p == pi && *i == ii) {
                        self.search_matches.push((pi, ii, item_score));
                    }
                }
            }
        }

        // Sort by score (highest first)
        self.search_matches.sort_by(|a, b| b.2.cmp(&a.2));

        if self.search_match_index >= self.search_matches.len() {
            self.search_match_index = 0;
        }
    }

    fn jump_to_match(&mut self) {
        if let Some(&(pi, ii, _score)) = self.search_matches.get(self.search_match_index) {
            self.project_index = pi;
            self.item_index = ii;
            self.focus_left = false;
        }
    }

    fn is_search_match(&self, project_idx: usize, item_idx: usize) -> bool {
        self.search_matches.iter().any(|(pi, ii, _)| *pi == project_idx && *ii == item_idx)
    }

    fn is_project_search_match(&self, project_idx: usize) -> bool {
        self.search_matches.iter().any(|(pi, _, _)| *pi == project_idx)
    }

    fn open_viewer(&mut self) {
        if let Some(item) = self.current_item() {
            let path = match item {
                MemoryItem::Session { path, .. } => path.join("conversation.md"),
                MemoryItem::KnowledgeFile { path, .. } => path.clone(),
            };
            self.viewer_content = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                format!("Error reading {}: {}", path.display(), e)
            });
            self.scroll_offset = 0;
            self.screen = Screen::Viewer;
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        loop {
            terminal.draw(|f| match self.screen {
                Screen::Browser => ui::render_browser(f, self),
                Screen::Viewer => ui::render_viewer(f, self),
                Screen::Packs => ui::render_packs(f, self),
                Screen::PackDetail => ui::render_pack_detail(f, self),
            })?;

            if let Event::Key(key) = event::read()? {
                match self.screen {
                    Screen::Browser => {
                        if self.search_mode {
                            self.handle_search_keys(key.code);
                        } else if self.show_delete {
                            self.handle_delete_keys(key.code);
                        } else if self.handle_browser_keys(key.code, key.modifiers) {
                            return Ok(());
                        }
                    }
                    Screen::Viewer => {
                        self.handle_viewer_keys(key.code, terminal)?;
                    }
                    Screen::Packs => {
                        if self.pack_action_message.is_some() {
                            // Any key clears the message
                            self.pack_action_message = None;
                        } else if self.pack_search_mode {
                            self.handle_pack_search_keys(key.code);
                        } else if self.show_pack_confirm.is_some() {
                            self.handle_pack_confirm_keys(key.code);
                        } else if self.handle_packs_keys(key.code) {
                            return Ok(());
                        }
                    }
                    Screen::PackDetail => {
                        self.handle_pack_detail_keys(key.code, terminal)?;
                    }
                }
            }
        }
    }

    fn handle_search_keys(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.search_mode = false;
            }
            KeyCode::Enter => {
                self.search_mode = false;
                if !self.search_matches.is_empty() {
                    self.search_match_index = 0;
                    self.jump_to_match();
                }
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.compute_search_matches();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.compute_search_matches();
            }
            _ => {}
        }
    }

    fn handle_browser_keys(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return true,

            // Search
            KeyCode::Char('/') => {
                self.search_mode = true;
                self.search_query.clear();
                self.search_matches.clear();
                self.search_match_index = 0;
            }

            // Next/prev match
            KeyCode::Char('n') => {
                if !self.search_matches.is_empty() {
                    self.search_match_index =
                        (self.search_match_index + 1) % self.search_matches.len();
                    self.jump_to_match();
                }
            }
            KeyCode::Char('N') => {
                if !self.search_matches.is_empty() {
                    self.search_match_index = if self.search_match_index == 0 {
                        self.search_matches.len() - 1
                    } else {
                        self.search_match_index - 1
                    };
                    self.jump_to_match();
                }
            }

            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                if self.focus_left {
                    if self.project_index + 1 < self.tree.projects.len() {
                        self.project_index += 1;
                        self.item_index = 0;
                    }
                } else {
                    let count = self.project_item_count();
                    if self.item_index + 1 < count {
                        self.item_index += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.focus_left {
                    self.project_index = self.project_index.saturating_sub(1);
                    self.item_index = 0;
                } else {
                    self.item_index = self.item_index.saturating_sub(1);
                }
            }

            // Panel switching
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => {
                if self.focus_left && self.project_item_count() > 0 {
                    self.focus_left = false;
                }
            }
            KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => {
                self.focus_left = true;
            }

            // Open viewer
            KeyCode::Enter => {
                if !self.focus_left {
                    self.open_viewer();
                } else if self.project_item_count() > 0 {
                    // Switch to right panel on Enter in left panel
                    self.focus_left = false;
                }
            }

            // Delete
            KeyCode::Char('d') => {
                if !self.focus_left && self.current_item().is_some() {
                    self.show_delete = true;
                }
            }

            // Switch to Packs screen
            KeyCode::Char('p') => {
                self.screen = Screen::Packs;
                self.pack_index = 0;
            }

            _ => {}
        }
        false
    }

    fn handle_packs_keys(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') => return true,
            KeyCode::Esc => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.pack_index + 1 < self.packs.len() {
                    self.pack_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.pack_index = self.pack_index.saturating_sub(1);
            }
            KeyCode::Char('r') => {
                // Reload packs
                self.packs = data::load_packs(&self.memory_dir);
            }
            KeyCode::Char('/') => {
                // Enter search mode
                self.pack_search_mode = true;
                self.pack_search_query.clear();
                self.pack_search_matches.clear();
                self.pack_search_index = 0;
            }
            KeyCode::Char('n') => {
                // Next search match
                if !self.pack_search_matches.is_empty() {
                    self.pack_search_index = (self.pack_search_index + 1) % self.pack_search_matches.len();
                    self.jump_to_pack_match();
                }
            }
            KeyCode::Char('N') => {
                // Previous search match
                if !self.pack_search_matches.is_empty() {
                    self.pack_search_index = if self.pack_search_index == 0 {
                        self.pack_search_matches.len() - 1
                    } else {
                        self.pack_search_index - 1
                    };
                    self.jump_to_pack_match();
                }
            }
            KeyCode::Enter => {
                // View pack details
                self.open_pack_detail();
            }
            KeyCode::Char('u') => {
                // Update pack
                if !self.packs.is_empty() {
                    self.show_pack_confirm = Some(PackAction::Update);
                }
            }
            KeyCode::Char('d') => {
                // Uninstall pack
                if !self.packs.is_empty() {
                    self.show_pack_confirm = Some(PackAction::Uninstall);
                }
            }
            _ => {}
        }
        false
    }

    fn handle_pack_confirm_keys(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(action) = self.show_pack_confirm.clone() {
                    if let Some(pack) = self.packs.get(self.pack_index) {
                        let pack_name = pack.name.clone();
                        self.execute_pack_action(action, &pack_name);
                    }
                }
                self.show_pack_confirm = None;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.show_pack_confirm = None;
            }
            _ => {}
        }
    }

    fn execute_pack_action(&mut self, action: PackAction, pack_name: &str) {
        use crate::hive::PackInstaller;

        let installer = PackInstaller::new(&self.memory_dir);

        match action {
            PackAction::Update => {
                match installer.update(pack_name) {
                    Ok(_) => {
                        self.pack_action_message = Some((
                            format!("✓ Pack '{}' updated successfully", pack_name),
                            false,
                        ));
                        self.packs = data::load_packs(&self.memory_dir);
                    }
                    Err(e) => {
                        self.pack_action_message = Some((
                            format!("✗ Update failed: {}", e),
                            true,
                        ));
                    }
                }
            }
            PackAction::Uninstall => {
                match installer.uninstall(pack_name) {
                    Ok(_) => {
                        self.pack_action_message = Some((
                            format!("✓ Pack '{}' uninstalled", pack_name),
                            false,
                        ));
                        self.packs = data::load_packs(&self.memory_dir);
                        // Adjust index if needed
                        if self.pack_index >= self.packs.len() && !self.packs.is_empty() {
                            self.pack_index = self.packs.len() - 1;
                        }
                    }
                    Err(e) => {
                        self.pack_action_message = Some((
                            format!("✗ Uninstall failed: {}", e),
                            true,
                        ));
                    }
                }
            }
        }
    }

    fn open_pack_detail(&mut self) {
        if let Some(pack) = self.packs.get(self.pack_index) {
            self.pack_detail_content = data::render_pack_detail(pack, &self.memory_dir);
            self.pack_detail_scroll = 0;
            self.screen = Screen::PackDetail;
        }
    }

    fn handle_pack_detail_keys(
        &mut self,
        code: KeyCode,
        terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let page_size = terminal.size()?.height.saturating_sub(4);
        let total_lines = self.pack_detail_content.lines().count() as u16;

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Packs;
                self.pack_detail_content.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.pack_detail_scroll < total_lines {
                    self.pack_detail_scroll += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.pack_detail_scroll = self.pack_detail_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.pack_detail_scroll = self.pack_detail_scroll.saturating_add(page_size).min(total_lines);
            }
            KeyCode::PageUp => {
                self.pack_detail_scroll = self.pack_detail_scroll.saturating_sub(page_size);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.pack_detail_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.pack_detail_scroll = total_lines;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_delete_keys(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(item) = self.current_item() {
                    let path = item.path().to_path_buf();
                    let _ = data::delete_entry(&path);
                    self.show_delete = false;
                    self.reload_tree();
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.show_delete = false;
            }
            _ => {}
        }
    }

    fn handle_viewer_keys(
        &mut self,
        code: KeyCode,
        terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let page_size = terminal.size()?.height.saturating_sub(4);
        let total_lines = self.viewer_content.lines().count() as u16;

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Browser;
                self.viewer_content.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.scroll_offset < total_lines {
                    self.scroll_offset += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_offset = self.scroll_offset.saturating_add(page_size).min(total_lines);
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.scroll_offset = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.scroll_offset = total_lines;
            }
            _ => {}
        }
        Ok(())
    }
    fn handle_pack_search_keys(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.pack_search_mode = false;
            }
            KeyCode::Enter => {
                self.pack_search_mode = false;
                if !self.pack_search_matches.is_empty() {
                    self.pack_search_index = 0;
                    self.jump_to_pack_match();
                }
            }
            KeyCode::Backspace => {
                self.pack_search_query.pop();
                self.compute_pack_search_matches();
            }
            KeyCode::Char(c) => {
                self.pack_search_query.push(c);
                self.compute_pack_search_matches();
            }
            _ => {}
        }
    }

    fn compute_pack_search_matches(&mut self) {
        self.pack_search_matches.clear();

        if self.pack_search_query.is_empty() {
            return;
        }

        for (i, pack) in self.packs.iter().enumerate() {
            // Fuzzy match on pack name
            if let Some(_score) = self.fuzzy_matcher.fuzzy_match(&pack.name, &self.pack_search_query) {
                self.pack_search_matches.push(i);
                continue;
            }

            // Match on description
            if let Some(_score) = self.fuzzy_matcher.fuzzy_match(&pack.description, &self.pack_search_query) {
                self.pack_search_matches.push(i);
                continue;
            }

            // Match on keywords
            for keyword in &pack.keywords {
                if let Some(_score) = self.fuzzy_matcher.fuzzy_match(keyword, &self.pack_search_query) {
                    self.pack_search_matches.push(i);
                    break;
                }
            }

            // Match on categories
            for category in &pack.categories {
                if let Some(_score) = self.fuzzy_matcher.fuzzy_match(category, &self.pack_search_query) {
                    self.pack_search_matches.push(i);
                    break;
                }
            }
        }

        if self.pack_search_index >= self.pack_search_matches.len() {
            self.pack_search_index = 0;
        }
    }

    fn jump_to_pack_match(&mut self) {
        if let Some(&pack_idx) = self.pack_search_matches.get(self.pack_search_index) {
            self.pack_index = pack_idx;
        }
    }

    pub fn is_pack_search_match(&self, pack_idx: usize) -> bool {
        self.pack_search_matches.contains(&pack_idx)
    }
}

/// Entry point: set up terminal, run app, restore terminal.
pub fn run_tui(memory_dir: PathBuf) -> io::Result<()> {
    if !io::IsTerminal::is_terminal(&io::stdin()) {
        return Err(io::Error::other(
            "TUI requires an interactive terminal (stdin must be a TTY)",
        ));
    }

    // Set up panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(memory_dir);
    let result = app.run(&mut terminal);

    // Restore terminal
    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

