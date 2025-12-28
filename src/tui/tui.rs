use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, terminal,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    text::Text,
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};
use std::{io, thread, time::Duration};

use crate::player::audio_player::AudioPlayer;

pub struct Tui {
    player: AudioPlayer,
}

impl Tui {
    pub fn new(player: AudioPlayer) -> Self {
        Self { player }
    }

    pub fn run(&mut self) -> io::Result<()> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        self.setup_terminal(&mut terminal)?;

        loop {
            terminal.draw(|f| {
                let area = f.area();
                let chunks = self.layout_chunks(area);

                self.render_player(f, chunks[0]);
                self.render_controls(f, chunks[1]);
                self.render_shortcuts(f, chunks[2]);
            })?;

            // Sleep for a short duration to continuously update the progress bar
            thread::sleep(Duration::from_millis(100));

            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if self.handle_input(code)? {
                    break;
                }
            }
        }

        self.cleanup_terminal(&mut terminal)?;

        Ok(())
    }

    fn setup_terminal(
        &self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(terminal.backend_mut(), cursor::Hide)?;
        terminal.clear()?;
        execute!(
            terminal.backend_mut(),
            terminal::Clear(terminal::ClearType::Purge)
        )?;
        Ok(())
    }

    fn cleanup_terminal(
        &self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> io::Result<()> {
        terminal.clear()?;
        execute!(
            terminal.backend_mut(),
            terminal::Clear(terminal::ClearType::Purge)
        )?;
        terminal::disable_raw_mode()?;
        execute!(terminal.backend_mut(), cursor::Show)?;
        Ok(())
    }

    fn layout_chunks(&self, area: ratatui::layout::Rect) -> Vec<ratatui::layout::Rect> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
            ])
            .split(area)
            .to_vec() // Convert Rc<[Rect]> to Vec<Rect>
    }

    fn render_player(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let block = self.default_block("Audio Player");
        f.render_widget(block, area);

        // Call the method to render the progress bar
        self.render_progress_bar(f, area);
    }

    fn render_progress_bar(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let progress = self.player.progress();
        let gauge = Gauge::default()
            .gauge_style(ratatui::style::Style::default().fg(ratatui::style::Color::Green))
            .percent((progress * 100.0) as u16);

        // Set the gauge to a smaller area within the Audio Player block
        let progress_area = ratatui::layout::Rect {
            x: area.x + 1,
            y: area.y + 2,
            width: area.width - 2,
            height: 1,
        };
        f.render_widget(gauge, progress_area);
    }

    fn render_controls(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let play_pause_button = self.player.pause_or_play_button_text();
        let paragraph = Paragraph::new(play_pause_button).block(self.default_block("Control"));
        f.render_widget(paragraph, area);
    }

    fn render_shortcuts(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let controls = vec!["Space - Play/Pause", "R - Restart", "Esc or Q - Quit"];
        let controls_text = Text::from(controls.join("\n"));
        let shortcuts =
            Paragraph::new(controls_text).block(self.default_block("Keyboard Shortcuts"));
        f.render_widget(shortcuts, area);
    }

    fn handle_input(&mut self, code: KeyCode) -> io::Result<bool> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => Ok(true),
            KeyCode::Char(' ') => {
                self.player.toggle_playing();
                Ok(false)
            }
            KeyCode::Char('r') => {
                self.player.restart();
                self.player.toggle_playing();
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn default_block(&self, title: &str) -> Block {
        Block::default()
            .title(title.to_string())
            .borders(Borders::ALL) // Convert &str to String
    }
}
