# 🎧 Audio3DSP — 3D Spatial Audio Engine & Studio Console

> **Real-Time Lock-Free Stereo-to-3D Spatial Audio Processing & Multi-Band Vocal Presence Engine written in Rust.**

```
╔══════════════════════════════════════════════════════════════════════╗
║                     AUDIO-3D-DSP STUDIO CONSOLE                      ║
║  [CABLE Output] ──► [Parallel M/S Matrix] ──► [Headphones/Speakers]  ║
╚══════════════════════════════════════════════════════════════════════╝
```

`Audio3DSP` is a high-performance audio DSP application that intercepts desktop audio streams (e.g. Spotify, YouTube, games) via a virtual audio loopback, applies real-time parallel multi-band matrix processing, and outputs lush, wide 3D spatialized sound directly to your headphones or speakers.

---

## 📖 Table of Contents
- [User Guide (Non-Technical)](#-user-guide-non-technical)
  - [How It Works](#how-it-works)
  - [Quick Start Setup](#quick-start-setup)
  - [Command-Line Options](#command-line-options)
  - [Console Controls & Parameters](#console-controls--parameters)
- [Technical Specifications (Engineers & Developers)](#-technical-specifications-engineers--developers)
  - [DSP Architecture & Signal Flow](#dsp-architecture--signal-flow)
  - [Vocal "Emboss" Pipeline](#vocal-emboss-pipeline)
  - [Instrumental 3D Widening & Haas Delay](#instrumental-3d-widening--haas-delay)
  - [Real-Time Lock-Free Safety](#real-time-lock-free-safety)
  - [Buffer Size Tuning & Anti-Tremble Guide](#buffer-size-tuning--anti-tremble-guide)
- [Building & Running](#-building--running)

---

## 👤 User Guide (Non-Technical)

### How It Works
1. **Audio Capture**: Your computer routes sound (music, voice chat, game audio) through a virtual audio cable (**VB-Audio Virtual Cable**).
2. **DSP Processing**: `Audio3DSP` separates center sounds (like vocals) from side sounds (like wide instruments), elevates the vocals so they don't get lost in the mix, and expands the instruments outward into a 3D soundstage.
3. **Playback**: You hear high-fidelity, spatialized 3D audio in your headphones or speakers with zero perceptible latency.

---

### Quick Start Setup

1. **Install Virtual Audio Cable** (Windows):
   - Download and install [VB-Audio Virtual Cable](https://vb-audio.com/Cable/) (Free).
2. **Set Windows Audio Output**:
   - Change your Windows default playback device or application output (e.g., Spotify setting) to **"CABLE Input (VB-Audio Virtual Cable)"**.
3. **Launch Audio3DSP**:
   - Open your terminal and run `cargo run`.
   - The engine automatically detects `CABLE Output` as input and your `Headphones` / `Speakers` as output!

---

### Command-Line Options

You can customize how `Audio3DSP` launches using command-line arguments:

| Option / Flag | Description | Example |
| :--- | :--- | :--- |
| *(Default)* | Launches the native **Hardware Studio GUI Console** (Graphical window). | `cargo run` |
| `--cli` or `--tui` | Launches the interactive **Terminal Console** inside your command prompt. | `cargo run -- --cli` |
| `--buffer <SIZE>` or `-b <SIZE>` | Sets the hardware audio buffer size in frames (Default: `1024`). Higher buffer values prevent audio trembling/crackling. | `cargo run -- --buffer 2048` |

#### Combined Examples:
```bash
# Launch default GUI with extra buffer stability (2048 frames ~42ms)
cargo run -- --buffer 2048

# Launch Terminal TUI Console with 1024 buffer
cargo run -- --cli --buffer 1024
```

---

### Console Controls & Parameters

The mixing desk features 4 channel strips and stereo VU meters:

#### 1. `1: M/S STEREO WIDTH` (`1.0x` to `3.0x`, Default: `2.0x`)
- **What it does**: Expands the width of background instruments and ambient sounds outward away from your head.

#### 2. `2: HAAS DELAY` (`0.0 ms` to `40.0 ms`, Default: `10.0 ms`)
- **What it does**: Applies a micro-timing delay between the left and right ear channels to create a psychoacoustic perception of acoustic space.

#### 3. `3: SPATIAL REVERB` (`0%` to `100%`, Default: `25%`)
- **What it does**: Blends a lush simulated room reverb into the 3D space.

#### 4. `4: VOCAL EMBOSS` (`0.0 dB` to `+6.0 dB`, Default: `+3.0 dB`)
- **What it does**: Applies a presence EQ boost and soft saturation exclusively to center lead vocals, making the voice sound crisp, clear, and "embossed" over the music.

---

## 🛠 Technical Specifications (Engineers & Developers)

### DSP Architecture & Signal Flow

`Audio3DSP` employs a parallel multi-band matrix processing pipeline executed sample-by-sample inside the real-time `cpal` audio output callback:

```
[ Stereo Input (L, R) ]
          │
          ▼
   [ Mid/Side Split Matrix ]
   Mid = 0.5 * (L + R)
   Side = 0.5 * (L - R)
          │
   ┌──────┴──────────────────────────────────────┐
   │ (Mid Channel)                               │ (Side Channel)
   ▼                                             ▼
[ Vocal "Emboss" Pipeline ]              [ Instrumental 3D Widening ]
 ├─ 3 kHz Biquad Peaking EQ (+3 dB)       ├─ Side Gain Multiplier (1.0x - 3.0x)
 └─ Cubic Soft-Clipping Saturator         └─ Haas Micro-Delay Line (0 - 40 ms)
   │                                             │
   └──────┬──────────────────────────────────────┘
          │
          ▼
   [ Stereo Re-Matrix ]
   L_matrix = Mid' + Side'
   R_matrix = Mid' - Side'
          │
          ▼
   [ Stereo Reverb Space (fundsp) ]
          │
          ▼
   [ Output Compensation (+3 dB) & Peak Protection ]
          │
          ▼
[ Hardware Output Stream ]
```

---

### Vocal "Emboss" Pipeline
- **Parametric Peaking EQ**: Transposed Direct Form II Biquad Filter centered at $f_0 = 3000\text{ Hz}$ ($Q = 1.0$) providing $+0.0\text{ dB}$ to $+6.0\text{ dB}$ boost in the vocal presence zone.
- **Harmonic Exciter / Soft Saturator**: Polynomial cubic curve ($y = x - \frac{x^3}{3}$ for $|x| \le 1.0$) adding subtle odd harmonics to give lead vocals definition over dense instrument mixes.

### Systems-Level Performance Optimizations
- **MMCSS Real-Time Thread Priority (`src/main.rs`)**: On startup, `Audio3DSP` invokes Windows `AvSetMmThreadCharacteristicsW("Pro Audio")` via native FFI to elevate the OS thread priority above background processes and prevent scheduler preemptions.
- **SIMD Hardware Vectorization (`.cargo/config.toml`)**: Configured with `-C target-cpu=native` to allow LLVM to emit AVX2 / FMA SIMD vector instructions, processing multiple audio samples per CPU clock cycle in L1/L2 cache.
- **Zero Allocations in Callbacks**: All DSP filters, biquads, reverb delay lines, and buffers are pre-allocated at startup.
- **Thread Synchronization**: Parameter updates between the UI thread and WASAPI audio callback use lock-free atomic float bit-patterns (`AtomicU32` storing `f32::to_bits()`).
- **Ring Buffer**: Single-Producer Single-Consumer (SPSC) lock-free ring buffer (`ringbuf`) transports incoming audio frames from the capture thread to the playback thread.

---

### Buffer Size Tuning & Anti-Tremble Guide

If you experience trembling voice, stuttering, or crackling audio:
- **Root Cause**: WASAPI driver underrun (buffer size is smaller than OS context-switch scheduling interval).
- **Solution**: Pass a larger buffer size via `--buffer` flag:
  - `--buffer 512` (~10.6 ms) — Ultra-low latency (requires high-performance CPU / WASAPI Exclusive mode).
  - `--buffer 1024` (~21.3 ms) — **Default standard**, optimal balance of low latency & stability.
  - `--buffer 2048` (~42.6 ms) — Recommended if experiencing crackles or running virtual audio cables under high system load.

---

## 🚀 Building & Running

### Prerequisites
- [Rust Toolchain](https://www.rust-lang.org/tools/install) (`cargo`, `rustc` 1.75+)
- Windows OS (WASAPI support)
- VB-Audio Virtual Cable (or any loopback device)

### Commands

```bash
# Build release binary
cargo build --release

# Run with native Hardware Studio GUI
cargo run

# Run with Terminal TUI Console
cargo run -- --cli

# Run with 2048 frame buffer to prevent stuttering/trembling
cargo run -- --buffer 2048
```
