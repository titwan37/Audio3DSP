// Audio3DSP — Real-Time Stereo-to-3D Spatial Audio Processor
//
// Captures audio from a virtual loopback device (VB-Audio Virtual Cable),
// applies a DSP chain (M/S widening, Haas delay, spatial reverb), and
// outputs to physical headphones/speakers.
//
// Architecture:
//   [CABLE Output] → [Input Callback] → [Ring Buffer] → [Output Callback + DSP] → [Headphones]
//                                                              ↓
//                                                   [Atomic Shared Params & RMS]
//                                                              ↓
//                                                [eframe GUI Window / Ratatui TUI]
//
// REAL-TIME SAFETY:
//   - Ring buffer is lock-free SPSC (single-producer, single-consumer).
//   - Parameter sync and RMS level metering use lock-free AtomicU32 bit-packing.
//   - DSP chain is pre-allocated at startup; zero locks or heap allocations in callbacks.

mod devices;
mod dsp;
mod gui;
mod shared_state;
mod tui;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use shared_state::SharedParams;

#[cfg(target_os = "windows")]
mod mmcss {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "avrt")]
    extern "system" {
        fn AvSetMmThreadCharacteristicsW(
            task_name: *const u16,
            task_index: *mut u32,
        ) -> *mut std::ffi::c_void;
    }

    pub fn set_thread_priority_pro_audio() {
        let task_name: Vec<u16> = std::ffi::OsStr::new("Pro Audio")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut index = 0u32;
        unsafe {
            let handle = AvSetMmThreadCharacteristicsW(task_name.as_ptr(), &mut index);
            if !handle.is_null() {
                println!("[mmcss] Windows MMCSS 'Pro Audio' real-time thread priority enabled!");
            }
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let use_tui = args.iter().any(|arg| arg == "--tui" || arg == "--cli");

    #[cfg(target_os = "windows")]
    mmcss::set_thread_priority_pro_audio();

    // Parse requested buffer size from CLI args (e.g., --buffer 1024 or --buffer 2048)
    let mut requested_buffer_size: u32 = 1024; // Default to 1024 frames (~21ms at 48kHz) to prevent trembling/crackles
    for i in 0..args.len() {
        if (args[i] == "--buffer" || args[i] == "-b") && i + 1 < args.len() {
            if let Ok(val) = args[i + 1].parse::<u32>() {
                requested_buffer_size = val;
            }
        } else if args[i].starts_with("--buffer=") {
            if let Ok(val) = args[i]["--buffer=".len()..].parse::<u32>() {
                requested_buffer_size = val;
            }
        }
    }

    // ── Step 1: Initialize the platform audio host ──
    let host = cpal::default_host();

    // ── Step 2: Discover input (virtual cable) and output (headphones) devices ──
    let (input_device, output_device) = devices::find_devices(&host)?;

    // ── Step 3: Negotiate a common stream configuration ──
    let config = devices::negotiate_config(&input_device, &output_device, Some(requested_buffer_size))?;
    let sample_rate = config.sample_rate;

    // ── Step 4: Create lock-free shared state for UI ↔ Audio thread parameter sync & RMS metering ──
    let shared_params = SharedParams::new();
    let audio_shared_params = shared_params.clone();

    // ── Step 5: Create the lock-free ring buffer ──
    let ring_size = (sample_rate as usize) * 2;
    let ring = HeapRb::<f32>::new(ring_size);
    let (mut producer, mut consumer) = ring.split();

    // ── Step 6: Build the DSP processing chain ──
    let mut master_strip = dsp::MasterStrip::new(sample_rate);

    // ── Step 7: Build the INPUT stream ──
    let input_stream = input_device
        .build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let _written = producer.push_slice(data);
            },
            |err| {
                eprintln!("[input] Stream error: {}", err);
            },
            None,
        )
        .context("Failed to build input stream")?;

    // ── Step 8: Build the OUTPUT stream with Real-Time DSP & RMS Metering ──
    let output_stream = output_device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Poll atomic parameter updates from UI thread (lock-free)
                master_strip.chain.set_side_gain(audio_shared_params.get_side_gain());
                master_strip.chain.set_haas_delay_ms(audio_shared_params.get_haas_delay_ms());
                master_strip.chain.set_reverb_wet(audio_shared_params.get_reverb_wet());
                master_strip.chain.set_emboss_gain_db(audio_shared_params.get_emboss_gain_db());

                // Poll atomic bypass flags (zero allocation, lock-free)
                master_strip.chain.widener_enabled = audio_shared_params.is_widener_enabled();
                master_strip.chain.haas_enabled = audio_shared_params.is_haas_enabled();
                master_strip.chain.reverb_enabled = audio_shared_params.is_reverb_enabled();
                master_strip.chain.emboss_enabled = audio_shared_params.is_emboss_enabled();

                // Poll Pro-Audio parameters (lock-free)
                master_strip.utility.set_gain_db(audio_shared_params.get_fader_gain_db());
                master_strip.utility.set_mute(audio_shared_params.is_muted());
                master_strip.comp.set_threshold(audio_shared_params.get_comp_thresh());
                master_strip.comp.set_ratio(audio_shared_params.get_comp_ratio());
                master_strip.comp_enabled = audio_shared_params.is_comp_enabled();

                // Poll Live Venue Simulator parameters (lock-free)
                master_strip.saturation.set_drive(audio_shared_params.get_saturation_drive());
                master_strip.saturation.set_mix(audio_shared_params.get_saturation_mix());
                master_strip.saturation.enabled = audio_shared_params.is_saturation_enabled();

                master_strip.venue_expander.set_sensitivity(audio_shared_params.get_venue_sensitivity());
                master_strip.venue_expander.enabled = audio_shared_params.is_venue_enabled();

                master_strip.crossfeed.set_mix(audio_shared_params.get_crossfeed_mix());
                master_strip.crossfeed.enabled = audio_shared_params.is_crossfeed_enabled();

                let mut sum_sq_l = 0.0f32;
                let mut sum_sq_r = 0.0f32;
                let mut frame_count = 0usize;

                for frame in data.chunks_mut(2) {
                    let left_in = consumer.try_pop().unwrap_or(0.0);
                    let right_in = consumer.try_pop().unwrap_or(0.0);

                    let (left_out, right_out) = master_strip.process_frame(left_in, right_in);

                    sum_sq_l += left_out * left_out;
                    sum_sq_r += right_out * right_out;
                    frame_count += 1;

                    frame[0] = left_out;
                    if frame.len() > 1 {
                        frame[1] = right_out;
                    }
                }

                if frame_count > 0 {
                    let rms_l = (sum_sq_l / frame_count as f32).sqrt();
                    let rms_r = (sum_sq_r / frame_count as f32).sqrt();
                    audio_shared_params.set_rms(rms_l, rms_r);
                }
            },
            |err| {
                eprintln!("[output] Stream error: {}", err);
            },
            None,
        )
        .context("Failed to build output stream")?;

    // ── Step 9: Start both audio streams ──
    input_stream.play().context("Failed to start input stream")?;
    output_stream.play().context("Failed to start output stream")?;

    // ── Step 10: Dispatch UI loop based on CLI arguments ──
    if use_tui {
        println!("[main] Running Terminal TUI Console (Mode: CLI)...");
        let mut app = tui::TuiApp::new(shared_params);
        app.run()?;
    } else {
        println!("[main] Launching eframe Hardware Studio GUI window...");
        let native_options = eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_title("Audio3DSP — Live Concert Hardware Studio Console")
                .with_inner_size([1313.0, 769.0])
                .with_min_inner_size([840.0, 560.0]),
            ..Default::default()
        };

        let gui_shared_params = shared_params.clone();
        eframe::run_native(
            "Audio3DSP — Hardware Studio Console",
            native_options,
            Box::new(move |_cc| Ok(Box::new(gui::GuiApp::new(gui_shared_params)))),
        )
        .map_err(|e| anyhow::anyhow!("eframe window error: {}", e))?;
    }

    Ok(())
}
