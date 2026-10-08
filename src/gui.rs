// Audio3DSP — Hardware Studio GUI Console (egui / eframe)
//
// Hardware-emulated audio mixing desk featuring:
//   - Equal-width Channel Strips (160px width, 240px height)
//   - Perfectly aligned Master VU Meters card (180px width, 240px height)
//   - Rotative Potentiometer Knobs drawn via egui::Painter
//   - Lock-free atomic synchronization with the WASAPI audio thread

use eframe::egui::{
    self, Align, Align2, Color32, Frame, Layout, Margin, Pos2, Rect, Rounding, ScrollArea, Sense,
    Stroke, Vec2,
};
use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use crate::shared_state::SharedParams;

/// Main `eframe` GUI application window manager.
pub struct GuiApp {
    shared_params: Arc<SharedParams>,
    peak_left: f32,
    peak_right: f32,
}

impl GuiApp {
    pub fn new(shared_params: Arc<SharedParams>) -> Self {
        Self {
            shared_params,
            peak_left: 0.0,
            peak_right: 0.0,
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repainting for smooth 60 FPS VU meter updates
        ctx.request_repaint_after(Duration::from_millis(16));

        // Read live RMS levels from lock-free shared parameters
        let (rms_l, rms_r) = self.shared_params.get_rms();

        // Smooth peak decay calculation
        self.peak_left = (self.peak_left * 0.92).max(rms_l);
        self.peak_right = (self.peak_right * 0.92).max(rms_r);

        // Apply dark studio visual style
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(18, 20, 24);
        visuals.window_fill = Color32::from_rgb(24, 26, 32);
        ctx.set_visuals(visuals);

        // ── Top Header Panel (Studio Logo Banner) ──
        egui::TopBottomPanel::top("header_panel")
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(10, 12, 16))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(40, 45, 55)))
                    .inner_margin(Margin::same(12.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Studio Logo Badge
                    let (logo_rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 38.0), Sense::hover());
                    if ui.is_rect_visible(logo_rect) {
                        let painter = ui.painter();
                        painter.rect_filled(logo_rect, 6.0, Color32::from_rgb(0, 190, 230));
                        painter.text(
                            logo_rect.center(),
                            Align2::CENTER_CENTER,
                            "3D",
                            egui::FontId::proportional(16.0),
                            Color32::BLACK,
                        );
                    }

                    ui.add_space(10.0);

                    ui.vertical(|ui| {
                        ui.heading(
                            egui::RichText::new("AUDIO-3D-DSP STUDIO CONSOLE")
                                .color(Color32::from_rgb(0, 220, 255))
                                .strong()
                                .size(19.0),
                        );
                        ui.label(
                            egui::RichText::new("3D Spatial Audio Engine • Lock-Free WASAPI Pipeline")
                                .color(Color32::from_rgb(140, 150, 165))
                                .size(11.0),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("● ENGINE ACTIVE")
                                .color(Color32::from_rgb(80, 220, 120))
                                .strong()
                                .size(12.0),
                        );
                    });
                });
            });
        // ── Footer Bar ──
        egui::TopBottomPanel::bottom("footer")
            .frame(Frame::default().inner_margin(Margin::same(4.0)).fill(Color32::from_rgb(30, 32, 38)))
            .show(ctx, |ui| {
                let size = ctx.screen_rect().size();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("Window Dimensions: {:.0} x {:.0}", size.x, size.y))
                            .color(Color32::from_rgb(150, 160, 175))
                            .size(10.0),
                    );
                });
            });

        // ── Main Mixing Board Central Panel ──
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(20, 22, 28))
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ScrollArea::both().show(ui, |ui| {
                    // Calculate responsive sizing based on available window dimensions
                    let available_w = ui.available_width();
                    let available_h = ui.available_height();

                    // Master section needs ~365px (VU card 180px + fader ~120px + margins & separators)
                    let strips_area_w = (available_w - 365.0).max(460.0);
                    let col_gap = 10.0;
                    let strip_w = ((strips_area_w - (3.0 * col_gap)) / 4.0).clamp(110.0, 180.0);

                    // Responsive row heights
                    let extra_h = ((available_h - 580.0) * 0.15).clamp(0.0, 30.0);
                    let row1_h = 230.0 + extra_h;
                    let row2_h = 280.0 + extra_h;

                    ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                        // ── Left Rack: 2x4 Channel Strips Table Grid ──
                        ui.vertical(|ui| {
                            ui.heading(
                                egui::RichText::new("CHANNEL STRIPS")
                                    .color(Color32::from_rgb(200, 210, 220))
                                    .size(13.0)
                                    .strong(),
                            );
                            ui.add_space(8.0);

                            egui::Grid::new("channel_strips_grid")
                                .spacing(Vec2::new(col_gap, 10.0))
                                .min_col_width(strip_w)
                                .show(ui, |ui| {
                                    // ══ ROW 1: Strips 1, 2, 3, 4 ══

                                    // ── Strip 1: Stereo Width ──
                                    self.render_channel_strip(
                                        ui,
                                        "1: M/S WIDTH",
                                        "STEREO WIDENER",
                                        Color32::from_rgb(50, 205, 120),
                                        self.shared_params.is_widener_enabled(),
                                        strip_w,
                                        row1_h,
                                        || { self.shared_params.toggle_widener(); },
                                        |ui| {
                                            let mut val = self.shared_params.get_side_gain();
                                            let readout = format!("{:.2}x", val);
                                            if rotary_knob_ui(
                                                ui,
                                                &mut val,
                                                1.0..=3.0,
                                                "GAIN",
                                                readout,
                                                Color32::from_rgb(50, 205, 120),
                                            ) {
                                                self.shared_params.set_side_gain(val);
                                            }
                                        },
                                    );

                                    // ── Strip 2: Haas Delay ──
                                    self.render_channel_strip(
                                        ui,
                                        "2: HAAS DELAY",
                                        "TIMING DELAY",
                                        Color32::from_rgb(255, 185, 40),
                                        self.shared_params.is_haas_enabled(),
                                        strip_w,
                                        row1_h,
                                        || { self.shared_params.toggle_haas(); },
                                        |ui| {
                                            let mut val = self.shared_params.get_haas_delay_ms();
                                            let readout = format!("{:.1} ms", val);
                                            if rotary_knob_ui(
                                                ui,
                                                &mut val,
                                                0.0..=40.0,
                                                "TIME",
                                                readout,
                                                Color32::from_rgb(255, 185, 40),
                                            ) {
                                                self.shared_params.set_haas_delay_ms(val);
                                            }
                                        },
                                    );

                                    // ── Strip 3: Reverb Wet Mix ──
                                    self.render_channel_strip(
                                        ui,
                                        "3: SPATIAL REVERB",
                                        "ROOM WET MIX",
                                        Color32::from_rgb(220, 90, 240),
                                        self.shared_params.is_reverb_enabled(),
                                        strip_w,
                                        row1_h,
                                        || { self.shared_params.toggle_reverb(); },
                                        |ui| {
                                            let mut val = self.shared_params.get_reverb_wet();
                                            let readout = format!("{:.0}%", val * 100.0);
                                            if rotary_knob_ui(
                                                ui,
                                                &mut val,
                                                0.0..=1.0,
                                                "WET MIX",
                                                readout,
                                                Color32::from_rgb(220, 90, 240),
                                            ) {
                                                self.shared_params.set_reverb_wet(val);
                                            }
                                        },
                                    );

                                    // ── Strip 4: Vocal Emboss Presence ──
                                    self.render_channel_strip(
                                        ui,
                                        "4: VOCAL EMBOSS",
                                        "PRESENCE BOOST",
                                        Color32::from_rgb(0, 200, 255),
                                        self.shared_params.is_emboss_enabled(),
                                        strip_w,
                                        row1_h,
                                        || { self.shared_params.toggle_emboss(); },
                                        |ui| {
                                            let mut val = self.shared_params.get_emboss_gain_db();
                                            let readout = format!("+{:.1} dB", val);
                                            if rotary_knob_ui(
                                                ui,
                                                &mut val,
                                                0.0..=6.0,
                                                "EMBOSS",
                                                readout,
                                                Color32::from_rgb(0, 200, 255),
                                            ) {
                                                self.shared_params.set_emboss_gain_db(val);
                                            }
                                        },
                                    );

                                    ui.end_row();

                                    // ══ ROW 2: Strips 5, 6, 7, 8 ══

                                    // ── Strip 5: Dynamics (Compressor) ──
                                    self.render_channel_strip(
                                        ui,
                                        "5: DYNAMICS",
                                        "STEREO COMP",
                                        Color32::from_rgb(255, 100, 100),
                                        self.shared_params.is_comp_enabled(),
                                        strip_w,
                                        row2_h,
                                        || { self.shared_params.toggle_comp(); },
                                        |ui| {
                                            let mut thresh = self.shared_params.get_comp_thresh();
                                            let thresh_readout = format!("{:.0} dB", thresh);
                                            if rotary_knob_ui(ui, &mut thresh, -40.0..=0.0, "THRESH", thresh_readout, Color32::from_rgb(255, 100, 100)) {
                                                self.shared_params.set_comp_thresh(thresh);
                                            }
                                            let mut ratio = self.shared_params.get_comp_ratio();
                                            let ratio_readout = format!("{:.1}:1", ratio);
                                            if rotary_knob_ui(ui, &mut ratio, 1.0..=10.0, "RATIO", ratio_readout, Color32::from_rgb(255, 100, 100)) {
                                                self.shared_params.set_comp_ratio(ratio);
                                            }
                                        },
                                    );

                                    // ── Strip 6: Analog Console Saturation ──
                                    self.render_channel_strip(
                                        ui,
                                        "6: ANALOG GLUE",
                                        "CONSOLE TAPE",
                                        Color32::from_rgb(255, 150, 50),
                                        self.shared_params.is_saturation_enabled(),
                                        strip_w,
                                        row2_h,
                                        || { self.shared_params.toggle_saturation(); },
                                        |ui| {
                                            let mut drive = self.shared_params.get_saturation_drive();
                                            let drive_readout = format!("{:.1}x", drive);
                                            if rotary_knob_ui(ui, &mut drive, 0.5..=4.0, "DRIVE", drive_readout, Color32::from_rgb(255, 150, 50)) {
                                                self.shared_params.set_saturation_drive(drive);
                                            }
                                            let mut mix = self.shared_params.get_saturation_mix();
                                            let mix_readout = format!("{:.0}%", mix * 100.0);
                                            if rotary_knob_ui(ui, &mut mix, 0.0..=1.0, "MIX", mix_readout, Color32::from_rgb(255, 150, 50)) {
                                                self.shared_params.set_saturation_mix(mix);
                                            }
                                        },
                                    );

                                    // ── Strip 7: Dynamic Venue Expander ──
                                    self.render_channel_strip(
                                        ui,
                                        "7: VENUE BLOOM",
                                        "ROOM EXPANDER",
                                        Color32::from_rgb(180, 100, 255),
                                        self.shared_params.is_venue_enabled(),
                                        strip_w,
                                        row2_h,
                                        || { self.shared_params.toggle_venue(); },
                                        |ui| {
                                            let mut sens = self.shared_params.get_venue_sensitivity();
                                            let sens_readout = format!("{:.2}", sens);
                                            if rotary_knob_ui(ui, &mut sens, 0.0..=2.0, "SENS", sens_readout, Color32::from_rgb(180, 100, 255)) {
                                                self.shared_params.set_venue_sensitivity(sens);
                                            }
                                        },
                                    );

                                    // ── Strip 8: Headphone Crossfeed ──
                                    self.render_channel_strip(
                                        ui,
                                        "8: PA CROSSFEED",
                                        "HEADPHONE HRTF",
                                        Color32::from_rgb(60, 180, 240),
                                        self.shared_params.is_crossfeed_enabled(),
                                        strip_w,
                                        row2_h,
                                        || { self.shared_params.toggle_crossfeed(); },
                                        |ui| {
                                            let mut mix = self.shared_params.get_crossfeed_mix();
                                            let mix_readout = format!("{:.0}%", mix * 100.0);
                                            if rotary_knob_ui(ui, &mut mix, 0.0..=1.0, "FEED", mix_readout, Color32::from_rgb(60, 180, 240)) {
                                                self.shared_params.set_crossfeed_mix(mix);
                                            }
                                        },
                                    );

                                    ui.end_row();
                                });
                        });

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // ── Right Rack: Master VU Meters Card & Master Fader ──
                        ui.scope(|ui| {
                            ui.set_min_width(320.0);
                            ui.vertical(|ui| {
                                ui.heading(
                                    egui::RichText::new("MASTER VU METERS")
                                        .color(Color32::from_rgb(200, 210, 220))
                                        .size(13.0)
                                        .strong(),
                                );
                                ui.add_space(8.0);

                                // Dynamic height calculated to match or exceed the 2-row channel strip grid
                                let min_grid_h = row1_h + row2_h + 10.0;
                                let dynamic_card_h = (available_h - 16.0).clamp(min_grid_h, 800.0);
                                let meter_h = (dynamic_card_h - 60.0).clamp(180.0, 740.0);

                                ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                                    Frame::none()
                                        .fill(Color32::from_rgb(14, 16, 20))
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(45, 50, 60)))
                                        .rounding(Rounding::same(6.0))
                                        .inner_margin(Margin::same(12.0))
                                        .show(ui, |ui| {
                                            ui.set_min_width(180.0);
                                            ui.set_min_height(dynamic_card_h);

                                            ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                                                ui.add_space(2.0);

                                                // Left Meter Column
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(60.0, dynamic_card_h - 24.0),
                                                    Layout::top_down(Align::Center),
                                                    |ui| {
                                                        vertical_vu_meter_ui(ui, "LEFT [L]", rms_l, self.peak_left, meter_h);
                                                    },
                                                );

                                                // Center calibrated dB scale
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(28.0, dynamic_card_h - 24.0),
                                                    Layout::top_down(Align::Center),
                                                    |ui| {
                                                        db_scale_ui(ui, meter_h);
                                                    },
                                                );

                                                // Right Meter Column
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(60.0, dynamic_card_h - 24.0),
                                                    Layout::top_down(Align::Center),
                                                    |ui| {
                                                        vertical_vu_meter_ui(ui, "RIGHT [R]", rms_r, self.peak_right, meter_h);
                                                    },
                                                );
                                            });
                                        });

                                    ui.add_space(10.0);

                                    // ── Master Fader & Mute ──
                                    self.render_channel_strip(
                                        ui,
                                        "MASTER",
                                        "FADER / MUTE",
                                        Color32::from_rgb(220, 220, 220),
                                        !self.shared_params.is_muted(),
                                        strip_w.min(130.0),
                                        row1_h,
                                        || { self.shared_params.toggle_mute(); },
                                        |ui| {
                                            let mut fader = self.shared_params.get_fader_gain_db();
                                            let fader_readout = format!("{:.1} dB", fader);
                                            if rotary_knob_ui(ui, &mut fader, -20.0..=6.0, "GAIN", fader_readout, Color32::WHITE) {
                                                self.shared_params.set_fader_gain_db(fader);
                                            }
                                        },
                                    );
                                });
                            });
                        });
                    });
                });
            });
    }
}

impl GuiApp {
    fn render_channel_strip(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        subtitle: &str,
        accent: Color32,
        is_enabled: bool,
        strip_w: f32,
        min_h: f32,
        on_toggle: impl FnOnce(),
        add_knob: impl FnOnce(&mut egui::Ui),
    ) {
        Frame::none()
            .fill(Color32::from_rgb(28, 32, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(50, 55, 68)))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                // Responsive channel strip size
                ui.set_width(strip_w);
                ui.set_min_width(strip_w);
                ui.set_min_height(min_h);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .color(accent)
                            .strong()
                            .size(12.0),
                    );
                    ui.label(
                        egui::RichText::new(subtitle)
                            .color(Color32::from_rgb(120, 130, 145))
                            .size(9.0),
                    );

                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // Dim the knob slightly if module is bypassed
                    ui.add_enabled_ui(is_enabled, |ui| {
                        add_knob(ui);
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // ── Interactive ON/OFF Bypass Toggle Switch ──
                    let (toggle_text, text_color, border_color, fill_color) = if is_enabled {
                        ("[ ON ]", Color32::from_rgb(50, 220, 120), Color32::from_rgb(30, 140, 70), Color32::from_rgb(18, 38, 26))
                    } else {
                        ("[OFF]", Color32::from_rgb(220, 70, 70), Color32::from_rgb(120, 40, 40), Color32::from_rgb(38, 20, 20))
                    };

                    let toggle_btn = egui::Button::new(
                        egui::RichText::new(toggle_text)
                            .color(text_color)
                            .strong()
                            .size(11.0),
                    )
                    .fill(fill_color)
                    .stroke(Stroke::new(1.5, border_color))
                    .rounding(Rounding::same(4.0));

                    let btn_w = (strip_w - 20.0).clamp(70.0, 100.0);
                    if ui.add_sized([btn_w, 24.0], toggle_btn).clicked() {
                        on_toggle();
                    }

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(if is_enabled { "DSP ACTIVE" } else { "BYPASSED" })
                            .color(if is_enabled { Color32::from_rgb(90, 160, 120) } else { Color32::from_rgb(150, 100, 100) })
                            .size(8.5),
                    );
                });
            });
    }
}

/// Custom hardware-style Rotative Potentiometer Knob widget drawn via egui::Painter
fn rotary_knob_ui(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    label: &str,
    readout: String,
    accent_color: Color32,
) -> bool {
    let mut changed = false;
    let size = Vec2::splat(68.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::drag());

    // Drag gesture handling (drag up increases value, drag down decreases)
    if response.dragged() {
        let delta = -response.drag_delta().y * 0.006 * (range.end() - range.start());
        let new_val = (*value + delta).clamp(*range.start(), *range.end());
        if (new_val - *value).abs() > 1e-5 {
            *value = new_val;
            changed = true;
            response.mark_changed();
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = rect.center();
        let radius = rect.width() / 2.0 - 4.0;

        // Background dial shadow
        painter.circle_filled(center + Vec2::new(1.0, 2.0), radius, Color32::from_black_alpha(100));

        // Outer track arc showing minimum to current value
        let norm = ((*value - range.start()) / (range.end() - range.start())).clamp(0.0, 1.0);
        let start_angle = std::f32::consts::PI * 0.75;
        let end_angle = std::f32::consts::PI * 2.25;
        let current_angle = start_angle + norm * (end_angle - start_angle);

        // Dial outer rim base
        let is_hovered = response.hovered() || response.dragged();
        let rim_stroke = Stroke::new(
            if is_hovered { 2.5 } else { 1.5 },
            if is_hovered { Color32::WHITE } else { Color32::from_rgb(70, 75, 90) },
        );
        painter.circle_filled(center, radius, Color32::from_rgb(22, 25, 32));
        painter.circle_stroke(center, radius, rim_stroke);

        // Dial inner cap
        painter.circle_filled(center, radius * 0.65, Color32::from_rgb(34, 38, 48));

        // Draw indicator needle line
        let dir = Vec2::new(current_angle.cos(), current_angle.sin());
        let needle_start = center + dir * (radius * 0.22);
        let needle_end = center + dir * (radius * 0.85);
        painter.line_segment([needle_start, needle_end], Stroke::new(3.5, accent_color));

        // Knob center dot
        painter.circle_filled(center, 3.5, accent_color);
    }

    // Label and numerical value readout below knob
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(Color32::from_rgb(160, 170, 185))
                .size(9.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new(readout)
                .color(accent_color)
                .strong()
                .size(13.0),
        );
    });

    changed
}

/// Maps linear amplitude (0.0 to 1.0+) to normalized meter height (0.0 to 1.0) on a -60 dBFS to 0 dBFS scale.
#[inline(always)]
fn amplitude_to_db_norm(amp: f32) -> f32 {
    const MIN_DB: f32 = -60.0;
    const MAX_DB: f32 = 0.0;
    if amp <= 1e-4 {
        0.0
    } else {
        let db = 20.0 * amp.log10();
        ((db - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0)
    }
}

/// Centered calibrated dBFS scale markings aligned with the VU meters
fn db_scale_ui(ui: &mut egui::Ui, meter_height: f32) {
    ui.vertical_centered(|ui| {
        // Space matching label above meters ("LEFT [L]" is ~14px + 4px space)
        ui.add_space(18.0);

        let width = 28.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, meter_height), Sense::hover());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let ticks: [(f32, &str, Color32); 8] = [
                (0.0, "0", Color32::from_rgb(255, 80, 80)),
                (-6.0, "-6", Color32::from_rgb(255, 140, 60)),
                (-12.0, "-12", Color32::from_rgb(255, 200, 40)),
                (-18.0, "-18", Color32::from_rgb(240, 215, 60)),
                (-24.0, "-24", Color32::from_rgb(120, 200, 140)),
                (-36.0, "-36", Color32::from_rgb(90, 170, 120)),
                (-48.0, "-48", Color32::from_rgb(70, 130, 100)),
                (-60.0, "-∞", Color32::from_rgb(100, 110, 125)),
            ];

            let inner_rect = rect.shrink(3.0);
            for (db, label_str, color) in ticks {
                let norm = ((db - (-60.0)) / 60.0).clamp(0.0, 1.0);
                let y = inner_rect.max.y - norm * inner_rect.height();

                painter.line_segment(
                    [Pos2::new(rect.min.x, y), Pos2::new(rect.min.x + 3.0, y)],
                    Stroke::new(1.0, Color32::from_rgb(80, 85, 100)),
                );
                painter.line_segment(
                    [Pos2::new(rect.max.x - 3.0, y), Pos2::new(rect.max.x, y)],
                    Stroke::new(1.0, Color32::from_rgb(80, 85, 100)),
                );

                painter.text(
                    Pos2::new(rect.center().x, y),
                    Align2::CENTER_CENTER,
                    label_str,
                    egui::FontId::proportional(8.5),
                    color,
                );
            }
        }

        ui.add_space(18.0);
    });
}

/// Custom Vertical Master Stereo VU Meter widget with dynamic height and logarithmic dBFS scaling
fn vertical_vu_meter_ui(ui: &mut egui::Ui, label: &str, rms: f32, peak: f32, meter_height: f32) {
    ui.vertical_centered(|ui| {
        // Label above meter
        ui.label(
            egui::RichText::new(label)
                .color(Color32::from_rgb(170, 180, 195))
                .size(10.0)
                .strong(),
        );
        ui.add_space(4.0);

        let width = 34.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(width, meter_height), Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Meter background frame
            painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 14, 18));
            painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(40, 45, 55)));

            let inner_rect = rect.shrink(3.0);
            let num_segments = ((meter_height / 7.5).round() as usize).clamp(20, 80);
            let seg_height = inner_rect.height() / num_segments as f32;

            // Logarithmic dBFS scale mapping (-60 dBFS to 0 dBFS)
            let active_rms_norm = amplitude_to_db_norm(rms);
            let active_peak_norm = amplitude_to_db_norm(peak);

            let active_rms_segs = (active_rms_norm * num_segments as f32).round() as usize;
            let active_peak_seg = (active_peak_norm * num_segments as f32).round() as usize;

            for i in 0..num_segments {
                let y_bottom = inner_rect.max.y - (i as f32 * seg_height);
                let y_top = (y_bottom - (seg_height - 1.5)).max(inner_rect.min.y);
                let seg_rect = Rect::from_min_max(
                    Pos2::new(inner_rect.min.x, y_top),
                    Pos2::new(inner_rect.max.x, y_bottom),
                );

                let seg_pct = (i as f32 + 0.5) / num_segments as f32;
                let seg_db = -60.0 + seg_pct * 60.0;

                let seg_color = if seg_db >= -6.0 {
                    Color32::from_rgb(255, 60, 60)
                } else if seg_db >= -18.0 {
                    Color32::from_rgb(255, 200, 40)
                } else {
                    Color32::from_rgb(50, 210, 100)
                };

                if i < active_rms_segs {
                    painter.rect_filled(seg_rect, 1.0, seg_color);
                } else {
                    painter.rect_filled(seg_rect, 1.0, Color32::from_rgb(20, 24, 30));
                }

                if i + 1 == active_peak_seg && active_peak_seg > 0 {
                    painter.rect_filled(seg_rect, 1.0, Color32::WHITE);
                }
            }
        }

        ui.add_space(4.0);

        // dBFS numerical display below meter
        let dbfs = if rms > 1e-4 {
            20.0 * rms.log10()
        } else {
            -60.0
        };
        let db_str = if dbfs <= -59.5 {
            "-∞ dB".to_string()
        } else {
            format!("{:.1} dB", dbfs)
        };

        ui.label(
            egui::RichText::new(db_str)
                .color(if dbfs > -3.0 {
                    Color32::RED
                } else {
                    Color32::from_rgb(0, 200, 240)
                })
                .size(10.0)
                .monospace()
                .strong(),
        );
    });
}
