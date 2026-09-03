# Implementation Plan — Soundstudio TUI Mix Console (`ratatui` + `crossterm`)

Build a real-time, interactive Terminal User Interface (TUI) mixing console for the `Audio3DSP` application. The TUI will display an ASCII art logo banner, interactive channel strips (potentiometers/sliders) for DSP parameters, and real-time stereo VU meters driven by atomic RMS feedback from the high-priority WASAPI audio callback thread.

## User Review Required

> [!IMPORTANT]
> **Non-Blocking Real-Time Architecture**: Parameter updates (M/S Width, Haas Delay, Reverb Wet Mix) and VU Meter levels (RMS Left/Right) are synchronized between the UI thread and WASAPI audio callback using atomic bit-packing (`std::sync::atomic::AtomicU32`). Zero locks, zero mutexes, zero heap allocations occur inside the audio processing loop.

> [!NOTE]
> **Terminal Controls**:
> - `Left` / `Right` or `h` / `l`: Switch active slider focus (Slider 1: Width, Slider 2: Haas Delay, Slider 3: Reverb Wet).
> - `Up` / `Down` or `k` / `j` / `+` / `-`: Adjust selected potentiometer value.
> - `q` / `Esc` / `Ctrl+C`: Gracefully exit the TUI console and shutdown audio streams.

---

## Proposed Changes

### Dependencies

#### [MODIFY] [Cargo.toml](file:///c:/Dev/Audio3DSP/Cargo.toml)
- Add `ratatui = "0.29"` and `crossterm = "0.28"`.

---

### DSP Core Adjustments

#### [MODIFY] [haas.rs](file:///c:/Dev/Audio3DSP/src/dsp/haas.rs)
- Increase `MAX_DELAY_SAMPLES` from 64 to 2048 to support Haas delay up to 40ms at 48 kHz (40ms × 48 = 1920 samples).
- Add `set_delay_ms(&mut self, delay_ms: f32, sample_rate: u32)` to allow dynamic delay changes.

#### [MODIFY] [widener.rs](file:///c:/Dev/Audio3DSP/src/dsp/widener.rs)
- Add `set_side_gain(&mut self, side_gain: f32)` for dynamic stereo width control.

#### [MODIFY] [reverb.rs](file:///c:/Dev/Audio3DSP/src/dsp/reverb.rs)
- Add `set_wet_mix(&mut self, wet_mix: f32)` for dynamic wet/dry ratio control.

#### [MODIFY] [chain.rs](file:///c:/Dev/Audio3DSP/src/dsp/chain.rs)
- Add parameter update methods: `set_side_gain`, `set_haas_delay_ms`, `set_reverb_wet`.

---

### TUI & State Sharing Engine

#### [NEW] [shared_state.rs](file:///c:/Dev/Audio3DSP/src/shared_state.rs)
- Implement `SharedParams` containing atomic variables (`AtomicU32` storing IEEE 754 float bit-patterns):
  - `side_gain` (1.0x to 3.0x)
  - `haas_delay_ms` (0.0ms to 40.0ms)
  - `reverb_wet` (0.0 to 1.0)
  - `rms_left` (0.0 to 1.0)
  - `rms_right` (0.0 to 1.0)

#### [NEW] [tui.rs](file:///c:/Dev/Audio3DSP/src/ui/tui.rs)
- Terminal setup/teardown with `crossterm` raw mode and panic hook protection.
- Layout division using `ratatui` (`Layout::default().direction(Direction::Vertical)...`):
  - **Top Banner**: ASCII art logo `"AUDIO-3D-DSP"` in a stylized block frame.
  - **Middle Console**: 3 Channel Strips / Potentiometers with custom block character sliders `[===o===]`, labels, parameter values, and active selection border highlights.
  - **Right VU Meter Panel**: Dual vertical/horizontal stereo VU meters (Left/Right) with green/yellow/red color thresholds and dBFS RMS readout.

#### [MODIFY] [main.rs](file:///c:/Dev/Audio3DSP/src/main.rs)
- Integrate `SharedParams` shared across UI thread and audio output stream callback.
- Calculate peak/RMS level per buffer in the audio callback and update atomic RMS values.
- Poll shared atomic parameters inside the output callback to update `DspChain` parameters in real-time.
- Replace blocking `stdin().read_line()` with the interactive `ratatui` event loop (~30 FPS rendering loop).

---

## Verification Plan

### Automated Tests
- Run `cargo test` to verify all unit tests pass with updated Haas buffer sizes and dynamic setter logic.
- Run `cargo clippy -- -D warnings` to guarantee zero lints or warnings.

### Manual Verification
- Run `cargo run` to launch the interactive TUI mix table dashboard.
- Verify smooth rendering of ASCII logo, potentiometer sliders, and VU meters.
- Adjust sliders using arrow keys and verify real-time DSP parameter updates without audio stuttering or crackling.
- Exit gracefully with `q` / `Esc` and ensure terminal state is fully restored.
