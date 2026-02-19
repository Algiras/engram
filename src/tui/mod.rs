pub mod data;
mod ui;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::time::Duration;

use data::{MemoryItem, MemoryTree};

enum Screen {
    Browser,
    Viewer,
    Packs,
    PackDetail,
    Learning,
    Analytics,
    Health,
    Daemon,
    Config,
    InjectPreview,
    Timeline,
    Ask,
    Help,
    Vcs,
}

#[derive(Clone, PartialEq)]
pub enum PackAction {
    Uninstall,
    Update,
}

#[derive(Clone, PartialEq)]
pub enum TuiAction {
    Ingest,
    Regen,
    Inject,
    LearnSimulate,
    LearnOptimize,
    Doctor,
    CleanupExpired,
    GraphBuild,
    DaemonStart,
    DaemonStop,
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

    // Learning state
    learning_content: String,
    learning_scroll: u16,

    // Analytics state
    analytics_content: String,
    analytics_scroll: u16,
    analytics_days: u32,

    // Health state
    health_content: String,
    health_scroll: u16,

    // Daemon state
    daemon_content: String,
    daemon_scroll: u16,
    daemon_interval: u64,

    // Action state
    pub action_message: Option<(String, bool)>, // (message, is_error)
    pub show_action_confirm: Option<TuiAction>,
    pending_action: Option<(String, Vec<String>)>, // (label, cli args)

    // Config screen state
    config_llm_index: usize,
    config_embed_index: usize,
    config_focus_llm: bool,
    pub config_status: String,
    config_test_running: bool,
    config_model_input_mode: bool,
    pub config_model_input: String,
    pending_config_test: Option<crate::auth::providers::Provider>,
    // Model picker (shown when provider supports /v1/models)
    pub config_model_list: Vec<String>,
    config_model_list_mode: bool,
    config_model_list_index: usize,
    config_model_list_scroll: usize,
    pending_model_fetch: Option<crate::auth::providers::Provider>,
    // Inject preview state
    pub inject_entries: Vec<crate::inject::SmartEntry>,
    inject_preview_index: usize,
    inject_preview_signal: String,
    inject_preview_budget: usize,
    inject_preview_status: String,
    pending_inject_preview: bool,

    // Timeline state
    pub timeline_items: Vec<crate::tui::data::TimelineEntry>,
    timeline_index: usize,
    timeline_scroll: usize,

    // Ask screen state
    ask_query: String,
    ask_result: String,
    ask_loading: bool,
    ask_input_mode: bool,
    pending_ask: Option<(String, String)>, // (project, query)
    ask_scroll: u16,

    // VCS state
    vcs_commits: Vec<crate::vcs::CommitObject>,
    vcs_commit_index: usize,
    vcs_commit_scroll: usize,
    vcs_snapshot_content: String,
    vcs_snapshot_scroll: u16,
    vcs_status_line: String,
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
            learning_content: String::new(),
            learning_scroll: 0,
            analytics_content: String::new(),
            analytics_scroll: 0,
            analytics_days: 30,
            health_content: String::new(),
            health_scroll: 0,
            daemon_content: String::new(),
            daemon_scroll: 0,
            daemon_interval: 15,
            action_message: None,
            show_action_confirm: None,
            pending_action: None,
            config_llm_index: 0,
            config_embed_index: 0,
            config_focus_llm: true,
            config_status: String::new(),
            config_test_running: false,
            config_model_input_mode: false,
            config_model_input: String::new(),
            pending_config_test: None,
            config_model_list: Vec::new(),
            config_model_list_mode: false,
            config_model_list_index: 0,
            config_model_list_scroll: 0,
            pending_model_fetch: None,
            inject_entries: Vec::new(),
            inject_preview_index: 0,
            inject_preview_signal: String::new(),
            inject_preview_budget: 1500,
            inject_preview_status: String::new(),
            pending_inject_preview: false,
            timeline_items: Vec::new(),
            timeline_index: 0,
            timeline_scroll: 0,
            ask_query: String::new(),
            ask_result: String::new(),
            ask_loading: false,
            ask_input_mode: false,
            pending_ask: None,
            ask_scroll: 0,
            vcs_commits: Vec::new(),
            vcs_commit_index: 0,
            vcs_commit_scroll: 0,
            vcs_snapshot_content: String::new(),
            vcs_snapshot_scroll: 0,
            vcs_status_line: String::new(),
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

    fn load_timeline_data(&mut self) {
        self.timeline_items = data::load_timeline(&self.memory_dir);
        self.timeline_index = 0;
        self.timeline_scroll = 0;
    }

    fn load_vcs_data(&mut self) {
        let project = match self.tree.projects.get(self.project_index) {
            Some(p) => p.name.clone(),
            None => {
                self.vcs_status_line = "No project selected".to_string();
                self.vcs_commits.clear();
                return;
            }
        };
        let vcs = crate::vcs::MemoryVcs::new(&self.memory_dir, &project);
        if !vcs.is_initialized() {
            self.vcs_status_line = format!(
                "VCS not initialized — run: engram mem init --project {}",
                project
            );
            self.vcs_commits.clear();
            return;
        }
        // Load status line
        self.vcs_status_line = match vcs.status() {
            Ok(s) => {
                let head = s
                    .head_hash
                    .as_deref()
                    .map(|h| &h[..h.len().min(8)])
                    .unwrap_or("(none)");
                format!(
                    "branch: {}  HEAD: {}  +{} unstaged  {} staged  {} deleted",
                    s.current_branch,
                    head,
                    s.unstaged_new.len(),
                    s.staged.len(),
                    s.unstaged_removed.len()
                )
            }
            Err(e) => format!("Error: {}", e),
        };
        // Load commit log
        self.vcs_commits = vcs.log(None, 50, None).unwrap_or_default();
        self.vcs_commit_index = 0;
        self.vcs_commit_scroll = 0;
        self.vcs_snapshot_scroll = 0;
        self.load_vcs_snapshot();
    }

    fn load_vcs_snapshot(&mut self) {
        let project = match self.tree.projects.get(self.project_index) {
            Some(p) => p.name.clone(),
            None => return,
        };
        let commit = match self.vcs_commits.get(self.vcs_commit_index) {
            Some(c) => c.hash.clone(),
            None => {
                self.vcs_snapshot_content = "(no commits yet)".to_string();
                return;
            }
        };
        let vcs = crate::vcs::MemoryVcs::new(&self.memory_dir, &project);
        self.vcs_snapshot_content = vcs
            .show(&commit, None)
            .unwrap_or_else(|e| format!("Error: {}", e));
        self.vcs_snapshot_scroll = 0;
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
            if let Some(project_score) = self
                .fuzzy_matcher
                .fuzzy_match(&project.name, &self.search_query)
            {
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
                if let Some(item_score) = self
                    .fuzzy_matcher
                    .fuzzy_match(&item.display_label(), &self.search_query)
                {
                    // Only add if not already added from project match
                    if !self
                        .search_matches
                        .iter()
                        .any(|(p, i, _)| *p == pi && *i == ii)
                    {
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
        self.search_matches
            .iter()
            .any(|(pi, ii, _)| *pi == project_idx && *ii == item_idx)
    }

    fn is_project_search_match(&self, project_idx: usize) -> bool {
        self.search_matches
            .iter()
            .any(|(pi, _, _)| *pi == project_idx)
    }

    fn open_viewer(&mut self) {
        if let Some(item) = self.current_item() {
            let path = match item {
                MemoryItem::Session { path, .. } => path.join("conversation.md"),
                MemoryItem::KnowledgeFile { path, .. } => path.clone(),
            };
            self.viewer_content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| format!("Error reading {}: {}", path.display(), e));
            self.scroll_offset = 0;
            self.screen = Screen::Viewer;
        }
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            // Execute pending CLI action if any (needs terminal access)
            if let Some((label, args)) = self.pending_action.take() {
                let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                let (output, success) = self.run_cli_command(terminal, &arg_refs)?;
                let msg = if success {
                    format!("{} completed successfully", label)
                } else {
                    let first_line = output.lines().last().unwrap_or("unknown error");
                    format!("{} failed: {}", label, first_line)
                };
                self.action_message = Some((msg, !success));
                // Reload data after action
                self.reload_tree();
                match self.screen {
                    Screen::Learning => self.load_learning_data(),
                    Screen::Analytics => self.load_analytics_data(),
                    Screen::Health => self.load_health_data(),
                    Screen::Daemon => self.load_daemon_data(),
                    Screen::Vcs => self.load_vcs_data(),
                    _ => {}
                }
            }

            terminal.draw(|f| match self.screen {
                Screen::Browser => ui::render_browser(f, self),
                Screen::Viewer => ui::render_viewer(f, self),
                Screen::Packs => ui::render_packs(f, self),
                Screen::PackDetail => ui::render_pack_detail(f, self),
                Screen::Learning => ui::render_learning(f, self),
                Screen::Analytics => ui::render_analytics(f, self),
                Screen::Health => ui::render_health(f, self),
                Screen::Daemon => ui::render_daemon(f, self),
                Screen::Config => ui::render_config(f, self),
                Screen::InjectPreview => ui::render_inject_preview(f, self),
                Screen::Timeline => ui::render_timeline(f, self),
                Screen::Ask => ui::render_ask(f, self),
                Screen::Help => ui::render_help(f, self),
                Screen::Vcs => ui::render_vcs(f, self),
            })?;

            // Execute pending config test (blocking HTTP call)
            if let Some(provider) = self.pending_config_test.take() {
                let result = crate::commands::provider_test::test_provider_sync(provider);
                self.config_test_running = false;
                self.config_status = if result.success {
                    format!(
                        "OK  {}ms  {}  \"{}\"",
                        result.latency_ms, result.model, result.response_snippet
                    )
                } else {
                    format!("FAIL: {}", result.error.as_deref().unwrap_or("unknown"))
                };
            }

            // Execute pending model fetch (blocking HTTP call to /v1/models)
            if let Some(provider) = self.pending_model_fetch.take() {
                match crate::commands::provider_test::fetch_models_sync(provider) {
                    Ok(models) if !models.is_empty() => {
                        self.config_model_list = models;
                        self.config_model_list_index = 0;
                        self.config_model_list_scroll = 0;
                        self.config_model_list_mode = true;
                        self.config_status = format!(
                            "{} models available — j/k: nav  Enter: select  Esc: cancel",
                            self.config_model_list.len()
                        );
                    }
                    Ok(_) => {
                        // Empty list — fall back to text input
                        self.config_model_input = self.current_config_model();
                        self.config_model_input_mode = true;
                        self.config_status = "No models returned — enter name manually".to_string();
                    }
                    Err(e) => {
                        // Error — fall back to text input
                        self.config_model_input = self.current_config_model();
                        self.config_model_input_mode = true;
                        self.config_status = format!("Fetch failed ({}) — enter name manually", e);
                    }
                }
            }

            // Execute pending ask query
            if let Some((project, query)) = self.pending_ask.take() {
                let args = vec!["ask", query.as_str(), "--project", project.as_str()];
                let (output, _) = self.run_cli_command(terminal, &args)?;
                self.ask_result = output;
                self.ask_loading = false;
            }

            // Execute pending smart inject preview load
            if self.pending_inject_preview {
                self.pending_inject_preview = false;
                let project = self.current_project_name().unwrap_or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .unwrap_or_default()
                });
                let home = dirs::home_dir().unwrap_or_default();
                let memory_dir = home.join("memory");
                let _search_result: Result<Vec<crate::inject::SmartEntry>, _> =
                    crate::inject::smart_search_sync(
                        &project,
                        &memory_dir,
                        &self.inject_preview_signal,
                        20,
                        0.45,
                    );
                match _search_result {
                    Ok(entries) if !entries.is_empty() => {
                        let total_tokens: usize = entries
                            .iter()
                            .map(|e: &crate::inject::SmartEntry| e.estimated_tokens())
                            .sum();
                        self.inject_preview_status = format!(
                            "{} entries · ~{} tokens · signal: {}",
                            entries.len(),
                            total_tokens,
                            &self.inject_preview_signal[..self.inject_preview_signal.len().min(60)]
                        );
                        self.inject_entries = entries;
                        self.inject_preview_index = 0;
                    }
                    Ok(_) => {
                        self.inject_preview_status =
                            "No embedding index — run 'engram embed <project>' first. Press q to go back.".to_string();
                        self.inject_entries.clear();
                    }
                    Err(e) => {
                        self.inject_preview_status = format!("Error: {} — press q to go back", e);
                        self.inject_entries.clear();
                    }
                }
            }

            // Poll with a 3-second timeout so Daemon screen auto-refreshes
            if !event::poll(Duration::from_secs(3))? {
                // Timeout — no key pressed
                if matches!(self.screen, Screen::Daemon) {
                    self.load_daemon_data();
                }
                continue;
            }

            if let Event::Key(key) = event::read()? {
                // Global: dismiss action message with any key
                if self.action_message.is_some() {
                    self.action_message = None;
                    continue;
                }
                // Global: handle action confirmation dialog
                if self.show_action_confirm.is_some() {
                    self.handle_action_confirm_keys(key.code);
                    continue;
                }

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
                    Screen::Learning => {
                        self.handle_learning_keys(key.code, terminal)?;
                    }
                    Screen::Analytics => {
                        self.handle_analytics_keys(key.code, terminal)?;
                    }
                    Screen::Health => {
                        self.handle_health_keys(key.code, terminal)?;
                    }
                    Screen::Daemon => {
                        self.handle_daemon_keys(key.code, terminal)?;
                    }
                    Screen::Config => {
                        self.handle_config_keys(key.code);
                    }
                    Screen::InjectPreview => {
                        self.handle_inject_preview_keys(key.code);
                    }
                    Screen::Timeline => {
                        if self.handle_timeline_keys(key.code) {
                            return Ok(());
                        }
                    }
                    Screen::Ask => {
                        self.handle_ask_keys(key.code);
                    }
                    Screen::Help => {
                        self.handle_help_keys(key.code)?;
                    }
                    Screen::Vcs => {
                        self.handle_vcs_keys(key.code);
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

            // Switch to Learning screen
            KeyCode::Char('L') => {
                self.load_learning_data();
                self.screen = Screen::Learning;
                self.learning_scroll = 0;
            }

            // Switch to Ask screen
            KeyCode::Char('A') => {
                self.ask_scroll = 0;
                self.screen = Screen::Ask;
            }

            // Switch to Health screen
            KeyCode::Char('H') => {
                self.load_health_data();
                self.screen = Screen::Health;
                self.health_scroll = 0;
            }

            // Switch to Daemon screen
            KeyCode::Char('D') => {
                self.load_daemon_data();
                self.screen = Screen::Daemon;
                self.daemon_scroll = 0;
            }

            // Switch to Timeline screen (Work log)
            KeyCode::Char('W') => {
                self.load_timeline_data();
                self.screen = Screen::Timeline;
            }

            // Show Help
            KeyCode::Char('?') => {
                self.screen = Screen::Help;
            }

            // Switch to VCS screen
            KeyCode::Char('V') => {
                self.load_vcs_data();
                self.screen = Screen::Vcs;
            }

            // Actions
            KeyCode::Char('i') => {
                self.show_action_confirm = Some(TuiAction::Ingest);
            }
            KeyCode::Char('R') => {
                self.show_action_confirm = Some(TuiAction::Regen);
            }
            KeyCode::Char('I') => {
                // Load smart inject preview instead of direct confirm
                let project = self.current_project_name().unwrap_or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .unwrap_or_default()
                });
                self.inject_preview_signal = crate::inject::detect_work_context(&project);
                self.inject_preview_budget = 1500;
                self.inject_preview_status = "Loading smart context...".to_string();
                self.pending_inject_preview = true;
                self.screen = Screen::InjectPreview;
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
                    self.pack_search_index =
                        (self.pack_search_index + 1) % self.pack_search_matches.len();
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
            KeyCode::Char('g') => {
                self.show_action_confirm = Some(TuiAction::GraphBuild);
            }
            _ => {
                self.handle_tab_switch(code);
            }
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
            PackAction::Update => match installer.update(pack_name) {
                Ok(_) => {
                    self.pack_action_message = Some((
                        format!("✓ Pack '{}' updated successfully", pack_name),
                        false,
                    ));
                    self.packs = data::load_packs(&self.memory_dir);
                }
                Err(e) => {
                    self.pack_action_message = Some((format!("✗ Update failed: {}", e), true));
                }
            },
            PackAction::Uninstall => {
                match installer.uninstall(pack_name) {
                    Ok(_) => {
                        self.pack_action_message =
                            Some((format!("✓ Pack '{}' uninstalled", pack_name), false));
                        self.packs = data::load_packs(&self.memory_dir);
                        // Adjust index if needed
                        if self.pack_index >= self.packs.len() && !self.packs.is_empty() {
                            self.pack_index = self.packs.len() - 1;
                        }
                    }
                    Err(e) => {
                        self.pack_action_message =
                            Some((format!("✗ Uninstall failed: {}", e), true));
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
                self.pack_detail_scroll = self
                    .pack_detail_scroll
                    .saturating_add(page_size)
                    .min(total_lines);
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
            _ => {
                self.handle_tab_switch(code);
            }
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
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_add(page_size)
                    .min(total_lines);
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
            _ => {
                self.handle_tab_switch(code);
            }
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
            if let Some(_score) = self
                .fuzzy_matcher
                .fuzzy_match(&pack.name, &self.pack_search_query)
            {
                self.pack_search_matches.push(i);
                continue;
            }

            // Match on description
            if let Some(_score) = self
                .fuzzy_matcher
                .fuzzy_match(&pack.description, &self.pack_search_query)
            {
                self.pack_search_matches.push(i);
                continue;
            }

            // Match on keywords
            for keyword in &pack.keywords {
                if let Some(_score) = self
                    .fuzzy_matcher
                    .fuzzy_match(keyword, &self.pack_search_query)
                {
                    self.pack_search_matches.push(i);
                    break;
                }
            }

            // Match on categories
            for category in &pack.categories {
                if let Some(_score) = self
                    .fuzzy_matcher
                    .fuzzy_match(category, &self.pack_search_query)
                {
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

    /// Handle global tab switching keys. Returns true if a tab switch occurred.
    /// Available from any screen: p=Packs, L=Learning, A=Analytics, H=Health, ?=Help, B=Browser
    fn handle_tab_switch(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('B') => {
                self.screen = Screen::Browser;
                true
            }
            KeyCode::Char('C') => {
                self.load_config_data();
                self.screen = Screen::Config;
                true
            }
            KeyCode::Char('p') => {
                self.screen = Screen::Packs;
                self.pack_index = 0;
                true
            }
            KeyCode::Char('L') => {
                self.load_learning_data();
                self.screen = Screen::Learning;
                self.learning_scroll = 0;
                true
            }
            KeyCode::Char('A') => {
                self.ask_scroll = 0;
                self.screen = Screen::Ask;
                true
            }
            KeyCode::Char('N') => {
                self.load_analytics_data();
                self.screen = Screen::Analytics;
                self.analytics_scroll = 0;
                true
            }
            KeyCode::Char('H') => {
                self.load_health_data();
                self.screen = Screen::Health;
                self.health_scroll = 0;
                true
            }
            KeyCode::Char('D') => {
                self.load_daemon_data();
                self.screen = Screen::Daemon;
                self.daemon_scroll = 0;
                true
            }
            KeyCode::Char('?') => {
                self.screen = Screen::Help;
                true
            }
            KeyCode::Char('V') => {
                self.load_vcs_data();
                self.screen = Screen::Vcs;
                true
            }
            _ => false,
        }
    }

    fn handle_ask_keys(&mut self, code: KeyCode) {
        if self.ask_input_mode {
            match code {
                KeyCode::Enter => {
                    if !self.ask_query.is_empty() {
                        self.ask_result = "Querying\u{2026}".to_string();
                        self.ask_loading = true;
                        let project = self
                            .current_project_name()
                            .unwrap_or_else(|| "default".to_string());
                        self.pending_ask = Some((project, self.ask_query.clone()));
                        self.ask_input_mode = false;
                    }
                }
                KeyCode::Esc => {
                    self.ask_input_mode = false;
                }
                KeyCode::Backspace => {
                    self.ask_query.pop();
                }
                KeyCode::Char(c) => {
                    self.ask_query.push(c);
                }
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Char('i') | KeyCode::Char('/') => {
                self.ask_input_mode = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.ask_scroll = self.ask_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ask_scroll = self.ask_scroll.saturating_sub(1);
            }
            KeyCode::Char('C') => {
                self.ask_query.clear();
                self.ask_result.clear();
                self.ask_scroll = 0;
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
    }

    fn handle_inject_preview_keys(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.inject_preview_index + 1 < self.inject_entries.len() {
                    self.inject_preview_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.inject_preview_index = self.inject_preview_index.saturating_sub(1);
            }
            KeyCode::Char(' ') => {
                // Toggle selection
                if let Some(e) = self.inject_entries.get_mut(self.inject_preview_index) {
                    e.selected = !e.selected;
                    if self.inject_preview_index + 1 < self.inject_entries.len() {
                        self.inject_preview_index += 1;
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // Toggle all
                let all_selected = self.inject_entries.iter().all(|e| e.selected);
                for e in &mut self.inject_entries {
                    e.selected = !all_selected;
                }
            }
            KeyCode::Enter => {
                // Execute inject with selected entries
                if self.inject_entries.is_empty() {
                    self.screen = Screen::Browser;
                    return;
                }
                let project = self.current_project_name().unwrap_or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                        .unwrap_or_default()
                });
                let home = dirs::home_dir().unwrap_or_default();
                let memory_dir = home.join("memory");
                let signal = self.inject_preview_signal.clone();
                let budget = self.inject_preview_budget;
                let entries = self.inject_entries.clone();

                match crate::inject::format_smart_memory(
                    &project,
                    &signal,
                    &entries,
                    budget,
                    &memory_dir,
                ) {
                    Ok(content) => {
                        // Find claude project dir and write
                        let claude_dir = home.join(".claude").join("projects");
                        let selected = entries.iter().filter(|e| e.selected).count();
                        // Write via engram inject --smart (spawn process to handle dir finding)
                        let result = std::process::Command::new("engram")
                            .args([
                                "inject",
                                "--smart",
                                "--budget",
                                &budget.to_string(),
                                &project,
                            ])
                            .output();
                        match result {
                            Ok(o) if o.status.success() => {
                                self.action_message = Some((
                                    format!(
                                        "Smart inject: {} entries written for '{}'",
                                        selected, project
                                    ),
                                    false,
                                ));
                            }
                            Ok(o) => {
                                let err = String::from_utf8_lossy(&o.stderr);
                                self.action_message = Some((
                                    format!(
                                        "Inject failed: {}",
                                        err.lines().last().unwrap_or("unknown")
                                    ),
                                    true,
                                ));
                            }
                            Err(e) => {
                                self.action_message = Some((format!("Inject error: {}", e), true));
                            }
                        }
                        let _ = content;
                        let _ = claude_dir;
                        self.screen = Screen::Browser;
                    }
                    Err(e) => {
                        self.action_message = Some((format!("Format error: {}", e), true));
                        self.screen = Screen::Browser;
                    }
                }
            }
            KeyCode::Char('+') => {
                self.inject_preview_budget = (self.inject_preview_budget + 500).min(8000);
            }
            KeyCode::Char('-') => {
                self.inject_preview_budget =
                    self.inject_preview_budget.saturating_sub(500).max(500);
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
    }

    fn handle_config_keys(&mut self, code: KeyCode) {
        use crate::auth::providers::Provider;

        // Model list picker mode
        if self.config_model_list_mode {
            self.handle_model_list_keys(code);
            return;
        }

        // Model input overlay mode
        if self.config_model_input_mode {
            match code {
                KeyCode::Enter => self.apply_model_input(),
                KeyCode::Esc => {
                    self.config_model_input_mode = false;
                    self.config_model_input.clear();
                }
                KeyCode::Backspace => {
                    self.config_model_input.pop();
                }
                KeyCode::Char(c) => {
                    self.config_model_input.push(c);
                }
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Tab => {
                self.config_focus_llm = !self.config_focus_llm;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.config_focus_llm {
                    let max = Provider::all().len().saturating_sub(1);
                    if self.config_llm_index < max {
                        self.config_llm_index += 1;
                    }
                } else if self.config_embed_index < 2 {
                    self.config_embed_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.config_focus_llm {
                    self.config_llm_index = self.config_llm_index.saturating_sub(1);
                } else {
                    self.config_embed_index = self.config_embed_index.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                if self.config_focus_llm {
                    self.set_default_provider();
                } else {
                    self.set_embed_provider();
                }
            }
            KeyCode::Char('T') => {
                self.config_status = "Testing...".to_string();
                self.config_test_running = true;
                let provider = self.current_config_provider();
                self.pending_config_test = Some(provider);
            }
            KeyCode::Char('M') => {
                let provider = self.current_config_provider();
                if provider.supports_model_list() {
                    self.config_status =
                        format!("Fetching models from {}...", provider.display_name());
                    self.pending_model_fetch = Some(provider);
                } else {
                    // Anthropic / Gemini — no /models endpoint, use text input
                    self.config_model_input = self.current_config_model();
                    self.config_model_input_mode = true;
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = Screen::Browser;
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
    }

    fn handle_model_list_keys(&mut self, code: KeyCode) {
        let count = self.config_model_list.len();
        match code {
            KeyCode::Esc => {
                self.config_model_list_mode = false;
                self.load_config_data();
            }
            KeyCode::Enter => {
                if let Some(model) = self.config_model_list.get(self.config_model_list_index) {
                    let model = model.clone();
                    self.save_model_for_current_provider(&model);
                    self.config_model_list_mode = false;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.config_model_list_index + 1 < count {
                    self.config_model_list_index += 1;
                    // Scroll window down when cursor approaches bottom
                    if self.config_model_list_index >= self.config_model_list_scroll + 18 {
                        self.config_model_list_scroll += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.config_model_list_index > 0 {
                    self.config_model_list_index -= 1;
                    if self.config_model_list_index < self.config_model_list_scroll {
                        self.config_model_list_scroll =
                            self.config_model_list_scroll.saturating_sub(1);
                    }
                }
            }
            KeyCode::PageDown => {
                self.config_model_list_index =
                    (self.config_model_list_index + 10).min(count.saturating_sub(1));
                self.config_model_list_scroll = self.config_model_list_index.saturating_sub(9);
            }
            KeyCode::PageUp => {
                self.config_model_list_index = self.config_model_list_index.saturating_sub(10);
                self.config_model_list_scroll = self.config_model_list_index;
            }
            _ => {}
        }
    }

    fn save_model_for_current_provider(&mut self, model: &str) {
        use crate::auth::{AuthStore, ProviderCredential};
        let provider = self.current_config_provider();
        if let Ok(mut store) = AuthStore::load() {
            if let Some(cred) = store.providers.get_mut(&provider.to_string()) {
                cred.model = Some(model.to_string());
            } else {
                store.set(
                    provider,
                    ProviderCredential {
                        cred_type: "api".to_string(),
                        key: String::new(),
                        endpoint: None,
                        model: Some(model.to_string()),
                    },
                );
            }
            if store.save().is_ok() {
                self.config_status =
                    format!("Set model for {} to {}", provider.display_name(), model);
            } else {
                self.config_status = "Error: failed to save".to_string();
            }
        }
    }

    fn load_config_data(&mut self) {
        let store = crate::auth::AuthStore::load().unwrap_or_default();
        let default = store.default_provider.as_deref().unwrap_or("none");
        let embed = store
            .embed_provider
            .as_deref()
            .unwrap_or("inferred from LLM");
        self.config_status = format!("Default LLM: {}  |  Embed: {}", default, embed);
    }

    pub fn current_config_provider(&self) -> crate::auth::providers::Provider {
        crate::auth::providers::Provider::all()[self.config_llm_index]
    }

    fn current_config_model(&self) -> String {
        use crate::auth::AuthStore;
        let provider = self.current_config_provider();
        let store = AuthStore::load().unwrap_or_default();
        store
            .get(provider)
            .and_then(|c| c.model.clone())
            .unwrap_or_else(|| provider.default_model().to_string())
    }

    fn set_default_provider(&mut self) {
        use crate::auth::AuthStore;
        let provider = self.current_config_provider();
        if let Ok(mut store) = AuthStore::load() {
            store.default_provider = Some(provider.to_string());
            if store.save().is_ok() {
                self.config_status =
                    format!("Set default LLM provider to {}", provider.display_name());
            } else {
                self.config_status = "Error: failed to save".to_string();
            }
        }
    }

    fn set_embed_provider(&mut self) {
        use crate::auth::AuthStore;
        let embed_providers = ["openai", "gemini", "ollama"];
        let name = embed_providers[self.config_embed_index];
        if let Ok(mut store) = AuthStore::load() {
            store.embed_provider = Some(name.to_string());
            if store.save().is_ok() {
                self.config_status = format!("Set embedding provider to {}", name);
            } else {
                self.config_status = "Error: failed to save".to_string();
            }
        }
    }

    fn apply_model_input(&mut self) {
        use crate::auth::{AuthStore, ProviderCredential};
        let model = self.config_model_input.trim().to_string();
        self.config_model_input_mode = false;
        if model.is_empty() {
            self.config_model_input.clear();
            return;
        }
        let provider = self.current_config_provider();
        if let Ok(mut store) = AuthStore::load() {
            if let Some(cred) = store.providers.get_mut(&provider.to_string()) {
                cred.model = Some(model.clone());
            } else {
                store.set(
                    provider,
                    ProviderCredential {
                        cred_type: "api".to_string(),
                        key: String::new(),
                        endpoint: None,
                        model: Some(model.clone()),
                    },
                );
            }
            if store.save().is_ok() {
                self.config_status =
                    format!("Set model for {} to {}", provider.display_name(), model);
            } else {
                self.config_status = "Error: failed to save".to_string();
            }
        }
        self.config_model_input.clear();
    }

    fn load_learning_data(&mut self) {
        if let Some(project) = self.tree.projects.get(self.project_index) {
            self.learning_content = data::load_learning_dashboard(&self.memory_dir, &project.name);
        } else {
            self.learning_content = "No project selected".to_string();
        }
    }

    fn load_analytics_data(&mut self) {
        if let Some(project) = self.tree.projects.get(self.project_index) {
            self.analytics_content =
                data::load_analytics(&self.memory_dir, &project.name, self.analytics_days);
        } else {
            self.analytics_content = "No project selected".to_string();
        }
    }

    fn load_health_data(&mut self) {
        if let Some(project) = self.tree.projects.get(self.project_index) {
            self.health_content = data::load_health_report(&self.memory_dir, &project.name);
        } else {
            self.health_content = "No project selected".to_string();
        }
    }

    fn load_daemon_data(&mut self) {
        // Read persisted interval from daemon.cfg if available
        let cfg_path = self.memory_dir.join("daemon.cfg");
        if let Ok(contents) = std::fs::read_to_string(&cfg_path) {
            if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(interval) = cfg.get("interval").and_then(|v| v.as_u64()) {
                    self.daemon_interval = interval;
                }
            }
        }
        self.daemon_content = data::load_daemon_status(&self.memory_dir);
    }

    fn handle_daemon_keys(
        &mut self,
        code: KeyCode,
        terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let page_size = terminal.size()?.height.saturating_sub(4);
        let total_lines = self.daemon_content.lines().count() as u16;

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('r') => {
                self.load_daemon_data();
                self.daemon_scroll = 0;
            }
            KeyCode::Char('s') => {
                self.show_action_confirm = Some(TuiAction::DaemonStart);
            }
            KeyCode::Char('x') => {
                self.show_action_confirm = Some(TuiAction::DaemonStop);
            }
            KeyCode::Char('+') | KeyCode::Char('>') => {
                self.daemon_interval = (self.daemon_interval + 5).min(120);
            }
            KeyCode::Char('-') | KeyCode::Char('<') => {
                self.daemon_interval = (self.daemon_interval.saturating_sub(5)).max(1);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.daemon_scroll < total_lines {
                    self.daemon_scroll += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.daemon_scroll = self.daemon_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.daemon_scroll = self
                    .daemon_scroll
                    .saturating_add(page_size)
                    .min(total_lines);
            }
            KeyCode::PageUp => {
                self.daemon_scroll = self.daemon_scroll.saturating_sub(page_size);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.daemon_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.daemon_scroll = total_lines;
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
        Ok(())
    }

    fn handle_learning_keys(
        &mut self,
        code: KeyCode,
        terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let page_size = terminal.size()?.height.saturating_sub(4);
        let total_lines = self.learning_content.lines().count() as u16;

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('r') => {
                self.load_learning_data();
                self.learning_scroll = 0;
            }
            KeyCode::Char('s') => {
                self.show_action_confirm = Some(TuiAction::LearnSimulate);
            }
            KeyCode::Char('o') => {
                self.show_action_confirm = Some(TuiAction::LearnOptimize);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.learning_scroll < total_lines {
                    self.learning_scroll += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.learning_scroll = self.learning_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.learning_scroll = self
                    .learning_scroll
                    .saturating_add(page_size)
                    .min(total_lines);
            }
            KeyCode::PageUp => {
                self.learning_scroll = self.learning_scroll.saturating_sub(page_size);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.learning_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.learning_scroll = total_lines;
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
        Ok(())
    }

    fn handle_analytics_keys(
        &mut self,
        code: KeyCode,
        terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let page_size = terminal.size()?.height.saturating_sub(4);
        let total_lines = self.analytics_content.lines().count() as u16;

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('r') => {
                self.load_analytics_data();
                self.analytics_scroll = 0;
            }
            KeyCode::Char('+') => {
                self.analytics_days = (self.analytics_days + 7).min(365);
                self.load_analytics_data();
                self.analytics_scroll = 0;
            }
            KeyCode::Char('-') => {
                self.analytics_days = self.analytics_days.saturating_sub(7).max(1);
                self.load_analytics_data();
                self.analytics_scroll = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.analytics_scroll < total_lines {
                    self.analytics_scroll += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.analytics_scroll = self.analytics_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.analytics_scroll = self
                    .analytics_scroll
                    .saturating_add(page_size)
                    .min(total_lines);
            }
            KeyCode::PageUp => {
                self.analytics_scroll = self.analytics_scroll.saturating_sub(page_size);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.analytics_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.analytics_scroll = total_lines;
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
        Ok(())
    }

    fn handle_health_keys(
        &mut self,
        code: KeyCode,
        terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        let page_size = terminal.size()?.height.saturating_sub(4);
        let total_lines = self.health_content.lines().count() as u16;

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('r') => {
                self.load_health_data();
                self.health_scroll = 0;
            }
            KeyCode::Char('x') => {
                self.show_action_confirm = Some(TuiAction::Doctor);
            }
            KeyCode::Char('c') => {
                self.show_action_confirm = Some(TuiAction::CleanupExpired);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.health_scroll < total_lines {
                    self.health_scroll += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.health_scroll = self.health_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.health_scroll = self
                    .health_scroll
                    .saturating_add(page_size)
                    .min(total_lines);
            }
            KeyCode::PageUp => {
                self.health_scroll = self.health_scroll.saturating_sub(page_size);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.health_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.health_scroll = total_lines;
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
        Ok(())
    }

    fn handle_vcs_keys(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('r') => {
                self.load_vcs_data();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.vcs_commit_index + 1 < self.vcs_commits.len() {
                    self.vcs_commit_index += 1;
                    if self.vcs_commit_index >= self.vcs_commit_scroll + 20 {
                        self.vcs_commit_scroll += 1;
                    }
                    self.load_vcs_snapshot();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.vcs_commit_index > 0 {
                    self.vcs_commit_index -= 1;
                    if self.vcs_commit_index < self.vcs_commit_scroll {
                        self.vcs_commit_scroll = self.vcs_commit_scroll.saturating_sub(1);
                    }
                    self.load_vcs_snapshot();
                }
            }
            KeyCode::Char('J') => {
                self.vcs_snapshot_scroll = self.vcs_snapshot_scroll.saturating_add(3);
            }
            KeyCode::Char('K') => {
                self.vcs_snapshot_scroll = self.vcs_snapshot_scroll.saturating_sub(3);
            }
            KeyCode::Char('c') => {
                // Commit all new sessions
                let project = match self.tree.projects.get(self.project_index) {
                    Some(p) => p.name.clone(),
                    None => return,
                };
                self.pending_action = Some((
                    "VCS Commit".to_string(),
                    vec![
                        "mem".to_string(),
                        "commit".to_string(),
                        project,
                        "-a".to_string(),
                        "-m".to_string(),
                        "snapshot from TUI".to_string(),
                    ],
                ));
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
    }

    fn handle_help_keys(&mut self, code: KeyCode) -> io::Result<()> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Browser;
            }
            _ => {
                self.handle_tab_switch(code);
            }
        }
        Ok(())
    }

    fn handle_action_confirm_keys(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(action) = self.show_action_confirm.take() {
                    self.execute_tui_action(action);
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.show_action_confirm = None;
            }
            _ => {}
        }
    }

    /// Get the currently selected project name, if any.
    fn current_project_name(&self) -> Option<String> {
        self.tree
            .projects
            .get(self.project_index)
            .map(|p| p.name.clone())
    }

    /// Run a engram CLI subcommand, suspending the TUI temporarily.
    /// Returns (stdout+stderr output, success bool).
    fn run_cli_command(
        &self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        args: &[&str],
    ) -> io::Result<(String, bool)> {
        // Leave alternate screen so user sees CLI output
        terminal::disable_raw_mode()?;
        crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

        print!("\n  Running: engram {}\n\n", args.join(" "));
        io::stdout().flush()?;

        let result = Command::new("engram").args(args).output();

        let (output_text, success) = match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Show output to terminal
                print!("{}", stdout);
                if !stderr.is_empty() {
                    eprint!("{}", stderr);
                }
                let combined = format!("{}{}", stdout, stderr);
                (combined, output.status.success())
            }
            Err(e) => (format!("Failed to run command: {}", e), false),
        };

        print!("\n  Press Enter to return to TUI...");
        io::stdout().flush()?;
        // Wait for Enter
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;

        // Re-enter alternate screen
        crossterm::execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;
        terminal.clear()?;

        Ok((output_text, success))
    }

    /// Execute a TUI action (called after confirmation).
    fn execute_tui_action(&mut self, action: TuiAction) {
        let project = self.current_project_name();
        let project_name = project.as_deref().unwrap_or("(none)");

        let (label, args): (&str, Vec<String>) = match &action {
            TuiAction::Ingest => (
                "Ingest",
                match &project {
                    Some(p) => vec!["ingest".into(), "--project".into(), p.clone()],
                    None => vec!["ingest".into()],
                },
            ),
            TuiAction::Regen => ("Regen", vec!["regen".into(), project_name.into()]),
            TuiAction::Inject => (
                "Inject",
                match &project {
                    Some(p) => vec!["inject".into(), p.clone()],
                    None => vec!["inject".into()],
                },
            ),
            TuiAction::LearnSimulate => (
                "Learn Simulate",
                vec!["learn".into(), "simulate".into(), project_name.into()],
            ),
            TuiAction::LearnOptimize => (
                "Learn Optimize",
                vec![
                    "learn".into(),
                    "optimize".into(),
                    project_name.into(),
                    "--auto".into(),
                ],
            ),
            TuiAction::Doctor => (
                "Doctor",
                match &project {
                    Some(p) => vec!["doctor".into(), p.clone()],
                    None => vec!["doctor".into()],
                },
            ),
            TuiAction::CleanupExpired => (
                "Cleanup Expired",
                vec!["forget".into(), project_name.into(), "--expired".into()],
            ),
            TuiAction::GraphBuild => (
                "Graph Build",
                vec!["graph".into(), "build".into(), project_name.into()],
            ),
            TuiAction::DaemonStart => (
                "Daemon Start",
                vec![
                    "daemon".into(),
                    "start".into(),
                    "--interval".into(),
                    self.daemon_interval.to_string(),
                ],
            ),
            TuiAction::DaemonStop => ("Daemon Stop", vec!["daemon".into(), "stop".into()]),
        };

        // Store the action details temporarily - actual execution happens in run() loop
        // where we have access to the terminal
        self.pending_action = Some((label.to_string(), args));
    }

    /// Handle key input on the Timeline screen. Returns true if the app should quit.
    fn handle_timeline_keys(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') => return true,
            KeyCode::Esc => {
                self.screen = Screen::Browser;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.timeline_index + 1 < self.timeline_items.len() {
                    self.timeline_index += 1;
                    // Auto-scroll
                    if self.timeline_index >= self.timeline_scroll + 20 {
                        self.timeline_scroll += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.timeline_index > 0 {
                    self.timeline_index -= 1;
                    if self.timeline_index < self.timeline_scroll {
                        self.timeline_scroll = self.timeline_index;
                    }
                }
            }
            KeyCode::Enter => {
                // Open viewer for current entry
                if let Some(entry) = self.timeline_items.get(self.timeline_index) {
                    self.viewer_content = format!(
                        "# {} / {} — {}\n\n{}\n",
                        entry.project, entry.category, entry.session_id, entry.content
                    );
                    self.scroll_offset = 0;
                    self.screen = Screen::Viewer;
                }
            }
            KeyCode::Char('r') => {
                self.load_timeline_data();
            }
            _ => {}
        }
        false
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
