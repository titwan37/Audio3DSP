Act as a Senior Audio Systems Engineer and systems programmer proficient in Rust. 

I need a standalone Rust command-line application that captures real-time stereo audio from a virtual loopback device (specifically targeting "CABLE Output" on Windows), applies custom DSP algorithms to create a 3D spatial stereophonic effect, and outputs the processed audio directly to a physical audio device (targeting "Headphones" or "Speakers").

Performance Requirements:
1. Complete Zero-Allocation Real-Time Loop: The audio thread callback must never invoke any allocations, deallocations, locks, or system calls to prevent dropouts (data discontinuities) and clicks.
2. Low Latency Execution: Maximize system scheduling performance. The setup must handle small buffer configurations (e.g., 512 or 1024 frames at 44100Hz) cleanly.

DSP Specifications:
1. Mid/Side (M/S) Matrix Widener: Decode the Left/Right input into Mid and Side. Provide a parameter to scale the Side channel volume (e.g., multiplier of 1.4) before encoding back to L/R.
2. Interaural Time Difference (ITD / Haas Effect): Introduce a micro-delay line (approx. 0.3ms to 0.7ms, adjustable in sample counts) exclusively to one channel to expand the psychoacoustic width.
3. Stereo Reverb Space: Integrate a lightweight spatial reverberation model. You may utilize `fundsp` filters or design a minimalist feedback delay network (FDN) / Schroeder reverberator natively in the thread loop.

Architecture Requirements:
- Use the `cpal` crate for cross-platform audio capturing and playback.
- Implement a lock-free, thread-safe Ring Buffer (like `ringbuf` or `rba`) to transport input audio frames from the capture callback to the playback DSP callback without context switching overhead.
- Provide automatic discovery to match device strings containing "CABLE" for input and "Headphones" or "Speakers" for output. Handle device termination or format mismatches gracefully.

Generate a single, thoroughly commented `main.rs` file alongside the necessary `Cargo.toml` dependencies structure. Ensure code handles boundaries correctly to avoid array panics inside the high-frequency loop callback.

---

To process and apply DSP effects like a 3D reverb to a live Spotify stream in real time without modifying the original song, you need an audio loopback or routing architecture.Spotify's official app does not allow third-party plugins to intercept its live audio output due to DRM (Digital Rights Management) and security restrictions. Therefore, Spotify's Pedalboard library cannot directly "hook" into the official Spotify desktop client while it is running.To achieve this, you must route the live desktop audio through a virtual audio device into your script, apply the DSP, and output it to your speakers.

The Architecture You Need[ Spotify Client ] 
       │
       ▼ (Output)
[ Virtual Audio Cable / BlackHole ]
       │
       ▼ (Input / Capture)
[ Your Python Script (SoundCard / PyAudio) ]
       │ 
       ▼ (Applies DSP: Pedalboard / Reverb)
[ System Audio Output (Speakers/Headphones) ]


Step-by-Step Implementation 

Guide1. 

Install a Virtual Audio RouterYou need a virtual driver to pass audio from Spotify to Python.Windows: Install VB-Audio Virtual Cable (Free).

---

import soundcard as sc

print("--- AVAILABLE MICROPHONES / INPUTS (Look for your Virtual Cable) ---")
for idx, mic in enumerate(sc.all_microphones()):
    print(f"[{idx}] {mic.name}")

print("\n--- AVAILABLE SPEAKERS / OUTPUTS (Look for your Headset) ---")
for idx, spk in enumerate(sc.all_speakers()):
    print(f"[{idx}] {spk.name}")

---

--- AVAILABLE MICROPHONES / INPUTS (Look for your Virtual Cable) ---
[0] CABLE Output (VB-Audio Virtual Cable)
[1] Microphone Array (Intel® Smart Sound Technology for Digital Microphones)

--- AVAILABLE SPEAKERS / OUTPUTS (Look for your Headset) ---
[0] CABLE In 16ch (VB-Audio Virtual Cable)
[1] Headphones (Realtek(R) Audio)
[2] CABLE Input (VB-Audio Virtual Cable)
[3] Speakers (Realtek(R) Audio)
[4] PHL 498P9 (2- HD Audio Driver for Display Audio)
-> Capturing: CABLE Output (VB-Audio Virtual Cable) | Outputting: Headphones (Realtek(R) Audio)

---

1. HRTF (Head-Related Transfer Function) / Binaural SpatializationThis is the most realistic way to achieve true 3D audio over headphones. HRTF uses complex digital filters to simulate exactly how a sound waves bounce off your human shoulders, head, and outer ear (pinna) before hitting your eardrums.How it sounds: Sounds can literally be placed behind, above, or below you, not just left and right.How to use it: You load an HRTF model or a 3D spatializer VST (like the free DearVR MICRO or Sennheiser AMBEO Orbit) into your pipeline.

2. Haas Effect (Haas Delay / Precedence Effect)This effect exploits how our brains determine sound direction based on arrival time. If the exact same mono sound is sent to both ears, but one ear is delayed by just a tiny fraction of a second, the brain perceives the sound as incredibly wide.How it sounds: Creates an massive sense of width, making the sound feel like it is stretching far beyond the physical boundaries of your headphones.The Trick: The delay must be between 10 to 35 milliseconds. If it is less than 10ms, it changes the tone (comb filtering). If it is more than 35ms, your brain hears it as a distinct echo instead of spatial width.

3. Mid/Side (M/S) Matrix ManipulationStereo audio tracks are made of a Mid channel (everything identical in both ears, like vocals and bass) and a Side channel (everything unique to the left or right ear, like wide guitars or ambient sounds).How it sounds: By boosting the volume of the Side channel and slightly reducing the Mid channel, you instantly push the music outward into a wide 3D sphere.Implementation: You split the stereo signal into Mid and Side components, apply a gain multiplier (e.g., Sides * 1.3), and recombine them back into Left/Right.

4. Interaural Time Difference (ITD) & Interaural Level Difference (ILD)These are the mathematical building blocks of panning.ITD: Adds a micro-delay (under 1ms) to one ear to simulate the physical distance between your left and right ear.ILD: Automatically dampens and reduces the high frequencies of the sound in the opposite ear to simulate your head blocking the sound wave (acoustic shadow).5. Chorus and Micro-Pitch ShiftingThis effect takes the audio stream, creates duplicates, slightly alters their pitch (by just a few cents), and pans them hard left and right.How it sounds: It creates a lush, shimmering, dimensional soundscape that mimics multiple sound sources playing at once in a physical room.


block_size = 4096  # Giving Python a much larger margin for error

The Structural Fix (Switch to Rust)

If Windows configuration doesn't solve it, Python's Global Interpreter Lock (GIL) and lack of direct low-level driver access are the bottlenecks. Implementing this application in Rust bypasses these limitations completely by writing directly to the low-latency hardware abstraction layers.Here is a minimalist Rust implementation setup using cpal (Cross-Platform Audio Library) and fundsp (a native, high-performance DSP library) to perform the exact same task.

Setup your Cargo.toml

Create a new Rust project (cargo new audio_dsp) and add these dependencies:toml[package]
name = "audio_dsp"
version = "0.1.0"
edition = "2021"

[dependencies]
cpal = "0.15"
fundsp = "0.18"


scr/main.rs

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fundsp::hacker::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();

    // 1. Find Audio Devices
    let devices = host.devices()?;
    let mut input_device = None;
    let mut output_device = None;

    for device in devices {
        if let Ok(name) = device.name() {
            if name.contains("CABLE Output") {
                input_device = Some(device);
            } else if name.contains("Headphones") {
                output_device = Some(device);
            }
        }
    }

    let input_device = input_device.expect("Virtual Cable not found");
    let output_device = output_device.expect("Headphones not found");

    // 2. Configure Stream Specs (44.1 kHz, Stereo)
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(44100),
        buffer_size: cpal::BufferSize::Fixed(1024), // Ultra-low latency buffer size
    };

    println!("Streaming audio smoothly with Rust... Press Enter to stop.");

    // 3. Define the DSP Graph using fundsp (Reverb room size, time)
    // pass() passes clean audio through, combined with a feedback delay/reverb network
    let mut reverb_unit = pass() >> reverb_stereo(10.0, 2.5, 0.5); 
    reverb_unit.set_sample_rate(44100.0);

    // Lock-free ring buffer to pass audio between input and output threads safely
    let (mut producer, mut consumer) = rba::ring_buffer::<f32>(8192);

    // 4. Input Recording Stream
    let input_stream = input_device.build_input_stream(
        &config,
        move |data: &[f32], _| {
            for &sample in data {
                let _ = producer.push(sample);
            }
        },

        |err| eprintln!("Input error: {}", err),
        None
    )?;

    // 5. Output Playback Stream (Where DSP happens in real-time)
    let output_stream = output_device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            for frame in data.chunks_mut(2) {
                let left_in = consumer.pop().unwrap_or(0.0);
                let right_in = consumer.pop().unwrap_or(0.0);
                
                // Process the frames through our DSP network
                let (left_out, right_out) = reverb_unit.filter(|left_in, right_in|);
                
                frame[0] = left_out;
                frame[1] = right_out;
            }
        },

        |err| eprintln!("Output error: {}", err),
        None
    )?;

    // Start streaming hardware clocks
    input_stream.play()?;
    output_stream.play()?;

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(())
}
