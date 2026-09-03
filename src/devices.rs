// Audio3DSP — Device Discovery & Configuration
//
// Scans all available audio devices, matches input (virtual cable)
// and output (headphones/speakers) by name substring, and negotiates
// a common stream configuration.

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host, StreamConfig, BufferSize};

/// Default name fragments used to match input and output devices.
const INPUT_MATCH_FRAGMENTS: &[&str] = &["CABLE Output", "CABLE"];
const OUTPUT_MATCH_FRAGMENTS: &[&str] = &["Headphones", "Speakers"];

/// Preferred sample rates, in order of priority.
const PREFERRED_SAMPLE_RATES: &[u32] = &[44100, 48000, 96000];

/// Preferred buffer sizes in frames, in order of priority (smallest first for lowest latency).
const PREFERRED_BUFFER_SIZES: &[u32] = &[512, 1024, 2048];

/// Get the human-readable name from a device, falling back to "<unknown>" on error.
fn device_name(device: &Device) -> String {
    device
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string())
}

/// Discover the input and output audio devices by matching name fragments.
///
/// Returns `(input_device, output_device)` or a descriptive error listing
/// all available devices if no match is found.
pub fn find_devices(host: &Host) -> Result<(Device, Device)> {
    let devices: Vec<Device> = host
        .devices()
        .context("Failed to enumerate audio devices")?
        .collect();

    // Collect all device names for error reporting
    let mut device_names: Vec<String> = Vec::new();
    let mut input_device: Option<Device> = None;
    let mut output_device: Option<Device> = None;

    for device in devices {
        let name = device_name(&device);
        device_names.push(name.clone());

        let has_input = device.supported_input_configs().map(|mut i| i.next().is_some()).unwrap_or(false);
        let has_output = device.supported_output_configs().map(|mut i| i.next().is_some()).unwrap_or(false);

        // Try to match input device first, then output.
        // Each device can only be claimed by one role.
        if input_device.is_none()
            && has_input
            && INPUT_MATCH_FRAGMENTS.iter().any(|f| name.contains(f))
        {
            println!("[devices] Input device matched: \"{}\"", name);
            input_device = Some(device);
        } else if output_device.is_none()
            && has_output
            && OUTPUT_MATCH_FRAGMENTS.iter().any(|f| name.contains(f))
        {
            println!("[devices] Output device matched: \"{}\"", name);
            output_device = Some(device);
        }
    }

    // Produce clear error messages if devices not found
    if input_device.is_none() || output_device.is_none() {
        let available = device_names
            .iter()
            .enumerate()
            .map(|(i, n)| format!("  [{}] {}", i, n))
            .collect::<Vec<_>>()
            .join("\n");

        if input_device.is_none() {
            bail!(
                "Could not find a virtual cable input device (looked for {:?}).\n\
                 Available devices:\n{}",
                INPUT_MATCH_FRAGMENTS,
                available
            );
        }
        bail!(
            "Could not find an output device (looked for {:?}).\n\
             Available devices:\n{}",
            OUTPUT_MATCH_FRAGMENTS,
            available
        );
    }

    Ok((input_device.unwrap(), output_device.unwrap()))
}

/// Negotiate a common `StreamConfig` that both the input and output devices support.
///
/// Prefers 44100 Hz, stereo (2 channels), with the smallest viable buffer size.
pub fn negotiate_config(input: &Device, output: &Device) -> Result<StreamConfig> {
    // Query supported configs from both devices
    let input_configs: Vec<_> = input
        .supported_input_configs()
        .context("Failed to query input device configs")?
        .collect();

    let output_configs: Vec<_> = output
        .supported_output_configs()
        .context("Failed to query output device configs")?
        .collect();

    // Find best common sample rate
    let sample_rate = find_common_sample_rate(&input_configs, &output_configs)
        .context("No common sample rate found between input and output devices")?;

    // Find best buffer size
    let buffer_size = find_best_buffer_size(&input_configs, &output_configs);

    let config = StreamConfig {
        channels: 2,
        sample_rate,
        buffer_size: match buffer_size {
            Some(size) => BufferSize::Fixed(size),
            None => BufferSize::Default,
        },
    };

    println!(
        "[devices] Negotiated config: {} Hz, {} ch, buffer: {:?}",
        sample_rate, config.channels, config.buffer_size
    );

    Ok(config)
}

/// Find the best sample rate supported by both input and output device config ranges.
fn find_common_sample_rate(
    input_configs: &[cpal::SupportedStreamConfigRange],
    output_configs: &[cpal::SupportedStreamConfigRange],
) -> Option<u32> {
    for &rate in PREFERRED_SAMPLE_RATES {
        let input_ok = input_configs
            .iter()
            .any(|c| c.min_sample_rate() <= rate && rate <= c.max_sample_rate() && c.channels() >= 2);
        let output_ok = output_configs
            .iter()
            .any(|c| c.min_sample_rate() <= rate && rate <= c.max_sample_rate() && c.channels() >= 2);

        if input_ok && output_ok {
            return Some(rate);
        }
    }
    None
}

/// Try to find the smallest preferred buffer size. Returns None if we should use default.
fn find_best_buffer_size(
    _input_configs: &[cpal::SupportedStreamConfigRange],
    _output_configs: &[cpal::SupportedStreamConfigRange],
) -> Option<u32> {
    // cpal's SupportedStreamConfigRange doesn't expose buffer size limits directly.
    // We'll try our preferred sizes and let cpal reject unsupported ones at stream build time.
    // Start with the smallest for lowest latency.
    Some(PREFERRED_BUFFER_SIZES[0])
}

#[cfg(test)]
mod tests {
    // Device discovery tests require real audio hardware and are therefore
    // manual integration tests rather than unit tests.
}
