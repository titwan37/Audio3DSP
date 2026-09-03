// Audio3DSP — Soundstudio TUI Mix Console
//
// Interactive mixing console rendering:
//   - Logo: ASCII art studio banner
//   - Channel Strips: Interactive potentiometers/sliders for DSP parameters
//   - Stereo VU Meters: Real-time RMS level meters with green/yellow/red color thresholds

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph};
use std::io::{Stdout, stdout};
use std::sync::Arc;
use std::time::Duration;

use crate::shared_state::SharedParams;

/// Active focused potentiometer slider in the mixing console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveSlider {
    StereoWidth = 0,
    HaasDelay = 1,
    ReverbWet = 2,
    VocalEmboss = 3,
}

impl ActiveSlider {
    pub fn next(self) -> Self {
        match self {
            Self::StereoWidth => Self::HaasDelay,
            Self::HaasDelay => Self::ReverbWet,
            Self::ReverbWet => Self::VocalEmboss,
            Self::VocalEmboss => Self::StereoWidth,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::StereoWidth => Self::VocalEmboss,
            Self::HaasDelay => Self::StereoWidth,
            Self::ReverbWet => Self::HaasDelay,
            Self::VocalEmboss => Self::ReverbWet,
        }
    }
}

struct SliderConfig<'a> {
    title: &'a str,
    val: f32,
    min: f32,
    max: f32,
    val_str: String,
    is_active: bool,
    accent_color: Color,
}

pub struct TuiApp {
    shared_params: Arc<SharedParams>,
    active_slider: ActiveSlider,
}

impl TuiApp {
    pub fn new(shared_params: Arc<SharedParams>) -> Self {
        Self {
            shared_params,
            active_slider: ActiveSlider::StereoWidth,
        }
    }

    /// Run the main TUI event and rendering loop.
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal raw mode and alternate screen
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Set panic hook to ensure terminal is restored on crash
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
            original_hook(panic_info);
        }));

        let loop_result = self.run_loop(&mut terminal);

        // Teardown terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        loop_result
    }

    fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        let frame_duration = Duration::from_millis(33); // ~30 FPS UI refresh rate

        loop {
            terminal.draw(|f| self.ui(f))?;

            if event::poll(frame_duration)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Left | KeyCode::Char('h') => {
                            self.active_slider = self.active_slider.prev();
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            self.active_slider = self.active_slider.next();
                        }
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('+') | KeyCode::Char('=') => {
                            self.adjust_value(1.0);
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('-') => {
                            self.adjust_value(-1.0);
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    fn adjust_value(&mut self, step_direction: f32) {
        match self.active_slider {
            ActiveSlider::StereoWidth => {
                let current = self.shared_params.get_side_gain();
                let next = (current + step_direction * 0.1).clamp(1.0, 3.0);
                self.shared_params.set_side_gain(next);
            }
            ActiveSlider::HaasDelay => {
                let current = self.shared_params.get_haas_delay_ms();
                let next = (current + step_direction * 1.0).clamp(0.0, 40.0);
                self.shared_params.set_haas_delay_ms(next);
            }
            ActiveSlider::ReverbWet => {
                let current = self.shared_params.get_reverb_wet();
                let next = (current + step_direction * 0.05).clamp(0.0, 1.0);
                self.shared_params.set_reverb_wet(next);
            }
            ActiveSlider::VocalEmboss => {
                let current = self.shared_params.get_emboss_gain_db();
                let next = (current + step_direction * 0.2).clamp(0.0, 6.0);
                self.shared_params.set_emboss_gain_db(next);
            }
        }
    }

    fn ui(&self, frame: &mut ratatui::Frame) {
        let size = frame.area();

        // Main Vertical Layout: Top Logo (7 lines), Middle Console (Min 12 lines), Bottom Help (3 lines)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),
                Constraint::Min(12),
                Constraint::Length(3),
            ])
            .split(size);

        self.render_logo(frame, main_chunks[0]);
        self.render_console(frame, main_chunks[1]);
        self.render_help(frame, main_chunks[2]);
    }

    /// Render ASCII Art Studio Logo Header
    fn render_logo(&self, frame: &mut ratatui::Frame, area: Rect) {
        let ascii_logo = vec![
            Line::from(vec![Span::styled(
                " █████╗ ██║   ██╗██████╗ ██╗██████╗     ██████╗    ██████╗ ███████╗██████╗ ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "██╔══██╗██║   ██║██╔══██╗██║██╔══██╗    ╚════██╗   ██╔══██╗██╔════╝██╔══██╗",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "███████║██║   ██║██║  ██║██║██║  ██║     █████╔╝   ██║  ██║███████╗██████╔╝",
                Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "██╔══██║██║   ██║██╔══██╗██║██║  ██║     ╚═══██╗   ██║  ██║╚════██║██╔═══╝ ",
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "██║  ██║╚██████╔╝██████╔╝██║██████╔╝    ██████╔╝██╗██████╔╝███████║██║     ",
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            )]),
        ];

        let logo_widget = Paragraph::new(ascii_logo)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Soundstudio Mix Console ")
                    .title_alignment(Alignment::Center),
            );

        frame.render_widget(logo_widget, area);
    }

    /// Render Middle Console: Left = Channel Strips (70%), Right = VU Meters (30%)
    fn render_console(&self, frame: &mut ratatui::Frame, area: Rect) {
        let console_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(area);

        self.render_channel_strips(frame, console_chunks[0]);
        self.render_vu_meters(frame, console_chunks[1]);
    }

    /// Render Channel Strips / Potentiometer Sliders
    fn render_channel_strips(&self, frame: &mut ratatui::Frame, area: Rect) {
        let strip_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
            ])
            .split(area);

        let width = self.shared_params.get_side_gain();
        let haas = self.shared_params.get_haas_delay_ms();
        let wet = self.shared_params.get_reverb_wet();
        let emboss = self.shared_params.get_emboss_gain_db();

        // 1. Stereo Width Slider
        self.render_slider(
            frame,
            strip_chunks[0],
            SliderConfig {
                title: "1: M/S WIDTH",
                val: width,
                min: 1.0,
                max: 3.0,
                val_str: format!("{:.1}x", width),
                is_active: self.active_slider == ActiveSlider::StereoWidth,
                accent_color: Color::Green,
            },
        );

        // 2. Haas Delay Slider
        self.render_slider(
            frame,
            strip_chunks[1],
            SliderConfig {
                title: "2: HAAS DELAY",
                val: haas,
                min: 0.0,
                max: 40.0,
                val_str: format!("{:.0} ms", haas),
                is_active: self.active_slider == ActiveSlider::HaasDelay,
                accent_color: Color::Yellow,
            },
        );

        // 3. Reverb Wet Mix Slider
        self.render_slider(
            frame,
            strip_chunks[2],
            SliderConfig {
                title: "3: REVERB WET",
                val: wet * 100.0,
                min: 0.0,
                max: 100.0,
                val_str: format!("{:.0}%", wet * 100.0),
                is_active: self.active_slider == ActiveSlider::ReverbWet,
                accent_color: Color::Magenta,
            },
        );

        // 4. Vocal Emboss Slider
        self.render_slider(
            frame,
            strip_chunks[3],
            SliderConfig {
                title: "4: VOCAL EMBOSS",
                val: emboss,
                min: 0.0,
                max: 6.0,
                val_str: format!("+{:.1} dB", emboss),
                is_active: self.active_slider == ActiveSlider::VocalEmboss,
                accent_color: Color::Cyan,
            },
        );
    }

    /// Render an individual potentiometer slider strip
    fn render_slider(&self, frame: &mut ratatui::Frame, area: Rect, cfg: SliderConfig) {
        let border_style = if cfg.is_active {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(if cfg.is_active { BorderType::Double } else { BorderType::Plain })
            .border_style(border_style)
            .title(Span::styled(
                format!(" {} ", cfg.title),
                if cfg.is_active {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let inner_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(4),
                Constraint::Length(3),
            ])
            .split(inner_area);

        let val_paragraph = Paragraph::new(Line::from(vec![
            Span::styled("Value: ", Style::default().fg(Color::Gray)),
            Span::styled(cfg.val_str, Style::default().fg(cfg.accent_color).add_modifier(Modifier::BOLD)),
        ]))
        .alignment(Alignment::Center);
        frame.render_widget(val_paragraph, inner_chunks[0]);

        let norm = ((cfg.val - cfg.min) / (cfg.max - cfg.min)).clamp(0.0, 1.0);
        let slider_width = (inner_chunks[1].width.saturating_sub(4)) as usize;
        let pos = (norm * (slider_width.saturating_sub(1) as f32)).round() as usize;

        let mut slider_bar = String::with_capacity(slider_width);
        for i in 0..slider_width {
            if i == pos {
                slider_bar.push('█');
            } else if i < pos {
                slider_bar.push('═');
            } else {
                slider_bar.push('─');
            }
        }

        let pot_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("[ {} ]", slider_bar),
                Style::default().fg(cfg.accent_color).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("Range: {:.1} .. {:.1}", cfg.min, cfg.max),
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let pot_paragraph = Paragraph::new(pot_lines).alignment(Alignment::Center);
        frame.render_widget(pot_paragraph, inner_chunks[1]);

        let focus_indicator = if cfg.is_active {
            Line::from(Span::styled(
                "◄ ACTIVE KNOB ►",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ))
        } else {
            Line::from(Span::styled(
                "Press ◄/► to select",
                Style::default().fg(Color::DarkGray),
            ))
        };
        let focus_paragraph = Paragraph::new(focus_indicator).alignment(Alignment::Center);
        frame.render_widget(focus_paragraph, inner_chunks[2]);
    }

    /// Render Real-Time Stereo VU Meters (Left / Right RMS)
    fn render_vu_meters(&self, frame: &mut ratatui::Frame, area: Rect) {
        let (rms_l, rms_r) = self.shared_params.get_rms();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" STEREO VU METERS ")
            .title_alignment(Alignment::Center);

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(inner_area);

        // Left Channel Gauge
        self.render_vu_gauge(frame, chunks[0], "LEFT  [L]", rms_l);

        // Right Channel Gauge
        self.render_vu_gauge(frame, chunks[1], "RIGHT [R]", rms_r);
    }

    /// Render a single channel VU gauge with green/yellow/red color thresholds
    fn render_vu_gauge(&self, frame: &mut ratatui::Frame, area: Rect, label: &str, rms: f32) {
        let pct = (rms * 100.0).clamp(0.0, 100.0) as u16;
        let dbfs = if rms > 1e-4 {
            20.0 * rms.log10()
        } else {
            -60.0
        };

        let gauge_color = if pct > 90 {
            Color::Red
        } else if pct > 70 {
            Color::Yellow
        } else {
            Color::Green
        };

        let db_str = if dbfs <= -59.0 {
            "-∞ dB".to_string()
        } else {
            format!("{:.1} dB", dbfs)
        };

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title(format!(" {} ({}) ", label, db_str))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .gauge_style(Style::default().fg(gauge_color).bg(Color::Black))
            .ratio(rms.clamp(0.0, 1.0) as f64)
            .label(format!("{}%", pct));

        frame.render_widget(gauge, area);
    }

    /// Render bottom control help bar
    fn render_help(&self, frame: &mut ratatui::Frame, area: Rect) {
        let help_text = vec![Line::from(vec![
            Span::styled(" Controls: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("[◄ / ► / h / l]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" Switch Channel  |  ", Style::default().fg(Color::Gray)),
            Span::styled("[▲ / ▼ / k / j / +/-]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" Turn Knob  |  ", Style::default().fg(Color::Gray)),
            Span::styled("[q / Esc]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(" Quit Console", Style::default().fg(Color::Gray)),
        ])];

        let help_widget = Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Plain)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );

        frame.render_widget(help_widget, area);
    }
}
