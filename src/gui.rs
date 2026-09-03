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
                    ui.horizontal(|ui| {
                        // ── Left Rack: 3 Equal-Width Channel Strips ──
                        ui.vertical(|ui| {
                            ui.heading(
                                egui::RichText::new("CHANNEL STRIPS")
                                    .color(Color32::from_rgb(200, 210, 220))
                                    .size(13.0)
                                    .strong(),
                            );
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                // ── Strip 1: Stereo Width ──
                                self.render_channel_strip(
                                    ui,
                                    "1: M/S WIDTH",
                                    "STEREO WIDENER",
                                    Color32::from_rgb(50, 205, 120),
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

                                ui.add_space(10.0);

                                // ── Strip 2: Haas Delay ──
                                self.render_channel_strip(
                                    ui,
                                    "2: HAAS DELAY",
                                    "TIMING DELAY",
                                    Color32::from_rgb(255, 185, 40),
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

                                ui.add_space(10.0);

                                // ── Strip 3: Reverb Wet Mix ──
                                self.render_channel_strip(
                                    ui,
                                    "3: SPATIAL REVERB",
                                    "ROOM WET MIX",
                                    Color32::from_rgb(220, 90, 240),
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

                                ui.add_space(10.0);

                                // ── Strip 4: Vocal Emboss Presence ──
                                self.render_channel_strip(
                                    ui,
                                    "4: VOCAL EMBOSS",
                                    "PRESENCE BOOST",
                                    Color32::from_rgb(0, 200, 255),
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
                            });
                        });

                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // ── Right Rack: Compact Master Stereo VU Meters Card ──
                        ui.vertical(|ui| {
                            ui.heading(
                                egui::RichText::new("MASTER METERS")
                                    .color(Color32::from_rgb(200, 210, 220))
                                    .size(13.0)
                                    .strong(),
                            );
                            ui.add_space(8.0);

                            Frame::none()
                                .fill(Color32::from_rgb(14, 16, 20))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(45, 50, 60)))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Margin::same(14.0))
                                .show(ui, |ui| {
                                    // Match the height of the channel strips (200px)
                                    ui.set_width(140.0);
                                    ui.set_height(200.0);

                                    ui.horizontal(|ui| {
                                        ui.add_space(6.0);

                                        // Left Meter Column (74px wide)
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(74.0, 230.0),
                                            Layout::top_down(Align::Center),
                                            |ui| {
                                                vertical_vu_meter_ui(ui, "LEFT [L]", rms_l, self.peak_left);
                                            },
                                        );

                                        ui.add_space(8.0);

                                        // Right Meter Column (74px wide)
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(74.0, 230.0),
                                            Layout::top_down(Align::Center),
                                            |ui| {
                                                vertical_vu_meter_ui(ui, "RIGHT [R]", rms_r, self.peak_right);
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
        add_knob: impl FnOnce(&mut egui::Ui),
    ) {
        Frame::none()
            .fill(Color32::from_rgb(28, 32, 40))
            .stroke(Stroke::new(1.0, Color32::from_rgb(50, 55, 68)))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(14.0))
            .show(ui, |ui| {
                // Strict fixed width (160px) and height (240px)
                ui.set_width(100.0);
                ui.set_height(170.0);
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

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    add_knob(ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new("Drag vertical to turn")
                            .color(Color32::from_rgb(100, 110, 125))
                            .size(9.0),
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

/// Custom Vertical Master Stereo VU Meter widget
fn vertical_vu_meter_ui(ui: &mut egui::Ui, label: &str, rms: f32, peak: f32) {
    ui.vertical_centered(|ui| {
        // Label above meter
        ui.label(
            egui::RichText::new(label)
                .color(Color32::from_rgb(170, 180, 195))
                .size(10.0)
                .strong(),
        );
        ui.add_space(4.0);

        let width = 36.0;
        let height = 175.0;
        let (rect, _response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Meter background frame
            painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 14, 18));
            painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(40, 45, 55)));

            let inner_rect = rect.shrink(3.0);
            let num_segments = 22;
            let seg_height = inner_rect.height() / num_segments as f32;

            let active_rms_segs = (rms.clamp(0.0, 1.0) * num_segments as f32).round() as usize;
            let active_peak_seg = (peak.clamp(0.0, 1.0) * num_segments as f32).round() as usize;

            for i in 0..num_segments {
                let y_bottom = inner_rect.max.y - (i as f32 * seg_height);
                let y_top = y_bottom - (seg_height - 1.5);
                let seg_rect = Rect::from_min_max(
                    Pos2::new(inner_rect.min.x, y_top),
                    Pos2::new(inner_rect.max.x, y_bottom),
                );

                let pct = (i as f32 / num_segments as f32) * 100.0;

                let seg_color = if pct > 90.0 {
                    Color32::from_rgb(255, 60, 60)
                } else if pct > 70.0 {
                    Color32::from_rgb(255, 200, 40)
                } else {
                    Color32::from_rgb(50, 210, 100)
                };

                if i < active_rms_segs {
                    painter.rect_filled(seg_rect, 1.0, seg_color);
                } else {
                    painter.rect_filled(seg_rect, 1.0, Color32::from_rgb(22, 26, 34));
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
        let db_str = if dbfs <= -59.0 {
            "-∞ dB".to_string()
        } else {
            format!("{:.1}dB", dbfs)
        };

        ui.label(
            egui::RichText::new(db_str)
                .color(if dbfs > -3.0 {
                    Color32::RED
                } else {
                    Color32::from_rgb(0, 200, 240)
                })
                .size(11.0)
                .strong(),
        );
    });
}
