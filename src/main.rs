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
//                                                    [Ratatui TUI Mix Console]
//
// REAL-TIME SAFETY:
//   - Ring buffer is lock-free SPSC (single-producer, single-consumer).
//   - Parameter sync and RMS level metering use lock-free AtomicU32 bit-packing.
//   - DSP chain is pre-allocated at startup; zero locks or heap allocations in callbacks.

mod devices;
mod dsp;
mod shared_state;
mod tui;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use shared_state::SharedParams;

fn main() -> Result<()> {
    // ── Step 1: Initialize the platform audio host ──
    let host = cpal::default_host();

    // ── Step 2: Discover input (virtual cable) and output (headphones) devices ──
    let (input_device, output_device) = devices::find_devices(&host)?;

    // ── Step 3: Negotiate a common stream configuration ──
    let config = devices::negotiate_config(&input_device, &output_device)?;
    let sample_rate = config.sample_rate;

    // ── Step 4: Create lock-free shared state for UI ↔ Audio thread parameter sync & RMS metering ──
    let shared_params = SharedParams::new();
    let audio_shared_params = shared_params.clone();

    // ── Step 5: Create the lock-free ring buffer ──
    // Size: ~1 second of stereo audio (sample_rate * 2 channels).
    let ring_size = (sample_rate as usize) * 2;
    let ring = HeapRb::<f32>::new(ring_size);
    let (mut producer, mut consumer) = ring.split();

    // ── Step 6: Build the DSP processing chain ──
    let mut dsp_chain = dsp::DspChain::new(sample_rate);

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
                dsp_chain.set_side_gain(audio_shared_params.get_side_gain());
                dsp_chain.set_haas_delay_ms(audio_shared_params.get_haas_delay_ms());
                dsp_chain.set_reverb_wet(audio_shared_params.get_reverb_wet());

                let mut sum_sq_l = 0.0f32;
                let mut sum_sq_r = 0.0f32;
                let mut frame_count = 0usize;

                // Process stereo frames (2 samples per frame: L, R)
                for frame in data.chunks_mut(2) {
                    let left_in = consumer.try_pop().unwrap_or(0.0);
                    let right_in = consumer.try_pop().unwrap_or(0.0);

                    // Run through the full DSP pipeline
                    let (left_out, right_out) = dsp_chain.process_frame(left_in, right_in);

                    // Accumulate energy for RMS calculation
                    sum_sq_l += left_out * left_out;
                    sum_sq_r += right_out * right_out;
                    frame_count += 1;

                    // Write processed samples to the output buffer
                    frame[0] = left_out;
                    if frame.len() > 1 {
                        frame[1] = right_out;
                    }
                }

                // Compute RMS levels for live VU meter rendering
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

    // ── Step 10: Run the interactive Soundstudio TUI Mix Console on the main thread ──
    let mut app = tui::TuiApp::new(shared_params);
    app.run()?;

    // Audio streams stop when input_stream and output_stream are dropped here
    Ok(())
}
