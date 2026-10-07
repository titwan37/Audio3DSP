# Audio3DSP — System Concept & Architecture Blueprint

## Executive Summary

`Audio3DSP` is a standalone, real-time audio processing system built in Rust that intercepts system-wide audio (Spotify, games, voice calls, DAW output) via a virtual loopback device (**VB-Audio Virtual Cable**), applies parallel matrix DSP transformations (Mid/Side widening, Haas micro-delay, vocal presence embossing, and Schroeder/Freeverb spatial diffusion), and streams spatialized 3D audio to headphones or speakers with zero perceptible latency and zero dropouts.

---

## 1. High-Level Routing Architecture

```
[ Audio Source (Spotify, Browser, Games) ]
                    │
                    ▼ (Default Windows Audio Playback)
  [ VB-Audio Virtual Cable (CABLE Input) ]
                    │
                    ▼ (Loopback Output Stream)
  [ CABLE Output (Virtual Capture Device) ]
                    │
                    ▼ (cpal Input Callback - Non-blocking push)
  ┌────────────────────────────────────────────────────────┐
  │  Lock-Free SPSC Ring Buffer (ringbuf HeapRb<f32>)      │
  └────────────────────────────────────────────────────────┘
                    │
                    ▼ (cpal Output Callback - Non-blocking pop)
  ┌────────────────────────────────────────────────────────┐
  │  Multi-Band Matrix Parallel DSP Engine                 │
  │   1. M/S Matrix Split                                  │
  │   2. Vocal "Emboss" Pipeline (Mid Channel)             │
  │   3. Instrumental 3D Widening (Side Channel)           │
  │   4. Re-Matrix (L/R)                                   │
  │   5. Static Schroeder/Freeverb Spatial Diffusion       │
  │   6. Peak Limiting & Output Gain Compensation          │
  └────────────────────────────────────────────────────────┘
                    │
                    ▼ (Lock-Free RMS & UI Parameter Sync)
  [ Physical Output (Headphones / Speakers) ]
```

---

## 2. Core DSP Specifications & Algorithms

### 2.1 Mid/Side (M/S) Matrix Widener
- **Algorithm**:
  $$\text{Mid} = 0.5 \times (L + R)$$
  $$\text{Side} = 0.5 \times (L - R)$$
  $$\text{Side}_{\text{wide}} = \text{Side} \times \text{Gain}_{\text{side}} \quad (\text{Range: } 1.0\times - 3.0\times)$$
- **Bypass Capability**: When bypassed, $\text{Side}_{\text{wide}} = \text{Side}$ without scaling arithmetic.

### 2.2 Haas Interaural Time Difference (ITD)
- **Algorithm**: Injects a circular buffer micro-delay ($\tau \in [0.0\text{ ms}, 40.0\text{ ms}]$) exclusively to the Side channel to create psychoacoustic depth and lateral spread without causing phase cancellation in mono center signals.
- **Bypass Capability**: When bypassed, the Side signal directly passes through with zero sample delay.

### 2.3 Vocal "Emboss" Pipeline (Center Channel)
- **Presence Boost**: Transposed Direct Form II Biquad Peaking EQ at $f_0 = 3000\text{ Hz}$ ($Q = 1.0$, boost $0\text{ dB} - +6\text{ dB}$) targeted specifically at the human speech/vocal intelligibility band.
- **Soft Saturation**: Cubic transfer function:
  $$f(x) = x - \frac{x^3}{3} \quad \text{for } |x| \le 1.0$$
  Adds subtle odd harmonics to give vocals distinct separation in dense instrumental mixes.
- **Bypass Capability**: When bypassed, the unprocessed clean $\text{Mid}$ signal routes directly to the matrix recombine stage.

### 2.4 Ultra-Low Latency Schroeder/Freeverb Spatial Reverb
- **Parallel Low-Pass Feedback Comb (LPFC) Filters**: 8 tuned filters per channel with mutually prime delay lengths ($1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617$ samples) and stereo spread offsets (+23 samples) for rich, non-metallic room reflection.
- **Cascaded All-Pass Diffusion**: 4 all-pass stages ($556, 441, 341, 225$ samples) providing smooth isotropic phase scattering.
- **Anti-Denormal Subnormal Protection**: Injected tiny DC offset ($1.0 \times 10^{-25}$) preventing x86 FPU microcode assist traps during quiet audio passages and decay tails.
- **100% Static Dispatch**: Completely eliminates dynamic trait `dyn AudioUnit` vtable calls, allowing full compiler inlining and auto-vectorization.
- **Bypass Capability**: When bypassed, skips all filter iterations and passes dry matrixed stereo directly.

---

## 3. Real-Time Safety & Concurrency Architecture

1. **Zero Allocations & Zero Locks**:
   - Audio callbacks never allocate or deallocate memory (`malloc` / `free`).
   - All delay buffers, comb filters, and ring buffers are statically or pre-allocated on initialization.
2. **Lock-Free Parameter Synchronization**:
   - All UI $\leftrightarrow$ Audio Thread parameter sharing is handled via lock-free atomic primitives:
     - `AtomicU32` storing `f32::to_bits()` and `f32::from_bits()` with `Ordering::Relaxed`.
     - `AtomicBool` for real-time DSP module bypass switches.
3. **Windows MMCSS Priority Elevation**:
   - The playback thread activates Windows `AvSetMmThreadCharacteristicsW("Pro Audio")` to ensure the audio processing loop is scheduled with high priority and never preempted by background OS tasks.
4. **Buffer Stability & Underrun Protection**:
   - SPSC ring buffer sized to $2 \times \text{SampleRate}$ (e.g. 96,000 samples) absorbs OS context switch jitter.
   - Configurable hardware buffer sizes (`--buffer 512`, `1024`, `2048`).
