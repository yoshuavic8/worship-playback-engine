# MASTER PROMPT — WORSHIP PLAYBACK ENGINE

You are a senior desktop application architect, audio software engineer, Rust/Tauri engineer, TypeScript/React engineer, and music technology engineer.

Your task is to build a **production-oriented local-first desktop application for church worship teams** called:

# Worship Playback Engine

Working tagline:

> **From Song to Sunday. Automatically.**

The application is intended for real-world church live worship environments where reliability, deterministic playback, low latency, offline capability, and simple operation are more important than flashy features.

The primary development machine is:

* Apple MacBook Air M4
* 24 GB unified memory
* macOS
* Apple Silicon
* Development environment: Cursor
* Desktop framework: Tauri 2
* Frontend: React + TypeScript
* Backend/orchestration: Rust
* Audio/AI preparation: Python
* Local database: SQLite
* Local LLM: Ollama
* No dependency on cloud AI APIs
* No required API keys
* No required subscription
* No required internet connection after installation and model/resource setup

---

# 1. PRODUCT VISION

Do NOT build another generic DAW.

Do NOT build another generic stem splitter.

Do NOT build a clone of Ableton Live.

Do NOT build a clone of Moises.

Build a specialized:

> **AI-assisted Worship Playback Engine**

The application takes a normal song:

MP3 / WAV / FLAC / M4A

and prepares it for live worship.

Typical workflow:

```text
Original Song
     ↓
AI Audio Preparation
     ↓
Vocals removed
Drums optionally removed
Bass optionally reduced
Other instruments retained
     ↓
Backing Track
     ↓
BPM / Beat Grid / Key / Sections detected
     ↓
Click + Cue generated
     ↓
FOH + Monitor routing configured
     ↓
Song becomes a Live-Ready Song
     ↓
Song added to Setlist
     ↓
MD controls entire set dynamically
```

The goal is to replace a workflow involving:

```text
Ultimate Vocal Remover
        ↓
Moises
        ↓
Ableton
        ↓
manual routing
        ↓
manual click/cue preparation
```

with:

```text
Worship Playback Engine
```

---

# 2. CORE PRODUCT PRINCIPLE

The most important architectural rule is:

> **AI may prepare the music. AI must NEVER be responsible for reliable live playback.**

The live playback engine must work even if:

* Ollama is unavailable
* Python is unavailable
* AI models are unavailable
* internet is unavailable
* an AI request fails
* metadata is incomplete

The application must therefore be divided into:

```text
                WORSHIP PLAYBACK ENGINE
                         │
        ┌────────────────┴────────────────┐
        │                                 │
 PREPARATION SYSTEM                  LIVE SYSTEM
        │                                 │
 AI / Python / Analysis              Native Audio
        │                                 │
 Demucs / MLX / FFmpeg               Deterministic
 BPM / Key / Sections                Low Latency
 Ollama (optional)                   Offline
        │                                 │
        └──────────────┬──────────────────┘
                       │
                  SQLite / Files
```

The live system must NEVER make an LLM request before playing audio.

---

# 3. TECHNOLOGY STACK

Use:

## Desktop

Tauri 2.

Use the current stable Tauri 2 architecture.

The application must run as a proper macOS `.app`.

Eventually it should support:

```text
Worship Playback Engine.app
```

with:

* native macOS window
* menu bar
* keyboard shortcuts
* drag and drop
* file dialogs
* audio device selection
* persistent application data
* native notifications where appropriate
* proper app data directory
* macOS packaging

Do not build this as a web app wrapped around a remote server.

Everything important must run locally.

---

# 4. FRONTEND

Use:

* React
* TypeScript
* Vite
* Tailwind CSS
* shadcn/ui where appropriate
* Lucide icons

Do NOT make the interface look like a generic admin dashboard.

The UI should feel like a combination of:

* professional music software
* modern macOS application
* worship production tool

Visual inspiration:

* Ableton Live for functional clarity
* Logic Pro for polish
* modern macOS apps for simplicity
* dark professional stage-production aesthetic

But DO NOT copy their UI.

---

# 5. UI/UX PRINCIPLES

The application must be:

### Easy

A volunteer who has never used a DAW should understand the basic workflow.

### Flexible

An experienced MD should be able to control:

* setlist
* songs
* sections
* click
* cue
* routing
* stem levels
* arrangement

### Safe

Live mode should deliberately remove unnecessary complexity.

### Professional

The interface should look credible on a church production computer.

### Fast

Do not require many dialogs.

### Offline

The core application must continue working without internet.

---

# 6. MAIN APPLICATION STRUCTURE

Use a sidebar navigation.

Suggested structure:

```text
Worship Playback Engine

├── Dashboard
├── Songs
├── Setlists
├── Live
├── Preparation
├── Devices
├── Settings
└── Diagnostics
```

The most important screens are:

1. Dashboard
2. Songs
3. Song Preparation
4. Setlist Editor
5. Live Mode
6. Device / Routing
7. Settings

---

# 7. DASHBOARD

Dashboard should show:

```text
Good morning, Yoshua

Upcoming Setlist
-----------------
Sunday Worship
5 songs

1. Goodness of God
2. Build My Life
3. What A Beautiful Name
4. ...
5. ...

[ Open Setlist ]

Recently Prepared
-----------------
Goodness of God
Ready

Build My Life
Processing

What A Beautiful Name
Ready
```

Also show:

```text
Audio Device
MacBook / Audio Interface

Output
48 kHz

Status
READY
```

Do not overload the dashboard.

---

# 8. SONG LIBRARY

Songs should be displayed as cards or compact rows.

Each song should show:

```text
Song Title
Artist
BPM
Key
Duration

Prepared ✓

Vocals     OFF
Drums      OFF
Bass       -6 dB

Click     ✓
Cue       ✓
```

Actions:

```text
Play
Edit
Prepare
Add to Setlist
Duplicate
Export
Delete
```

Support:

* drag & drop audio files
* MP3
* WAV
* FLAC
* M4A
* AIFF where supported

Use FFmpeg for conversion/normalization where necessary.

---

# 9. SONG PREPARATION

This is the heart of the application.

User should be able to:

```text
Drag audio file here
```

Then show:

```text
Song detected:

Title:
BPM:
Key:
Time Signature:
Duration:
Sample Rate:
```

Then:

# Worship Preparation

### Vocals

```text
REMOVE VOCALS
```

default ON.

### Drums

Options:

```text
REMOVE
KEEP
OPTIONAL
```

Default:

```text
REMOVE
```

### Bass

Use:

```text
Bass Level
-12 dB
-9 dB
-6 dB
-3 dB
0 dB
```

Default:

```text
-6 dB
```

### Other instruments

Default:

```text
0 dB
```

---

# 10. PRESET SYSTEM

Create presets.

## FULL BAND

```text
Vocals: REMOVE
Drums: REMOVE
Bass: -6 dB
Other: 0 dB
Click: ON
Cue: ON
```

## NO DRUMMER

```text
Vocals: REMOVE
Drums: KEEP
Bass: 0 dB
Other: 0 dB
Click: OFF
Cue: OFF
```

## NO BASS PLAYER

```text
Vocals: REMOVE
Drums: REMOVE
Bass: 0 dB
Other: 0 dB
Click: ON
Cue: ON
```

## FULL TRACK

```text
Vocals: REMOVE
Drums: KEEP
Bass: KEEP
Other: KEEP
Click: OFF
Cue: OFF
```

Allow custom presets.

---

# 11. STEM ARCHITECTURE

Do not unnecessarily create 20+ stems.

Target:

```text
vocals.wav
drums.wav
bass.wav
other.wav
```

Then generate:

```text
backing.wav
```

where:

```text
backing =
    other
  + adjusted bass
  + optional drums
```

The primary worship use case is:

```text
backing.wav
```

not individual stem manipulation.

Individual stems remain available for advanced editing.

---

# 12. AI AUDIO SEPARATION

Use a model abstraction layer.

Do NOT hard-code the entire application to one AI model.

Create:

```text
AudioSeparator
```

interface.

Example:

```typescript
interface AudioSeparator {
  separate(
    input: string,
    outputDirectory: string,
    options: SeparationOptions
  ): Promise<SeparationResult>
}
```

Implement:

```text
DemucsSeparator
DemucsMLXSeparator
FutureSeparator
```

The application should automatically detect Apple Silicon.

Preferred evaluation order:

1. Apple Silicon optimized Demucs/MLX implementation
2. Standard Demucs fallback
3. CPU fallback

Do not assume the MLX implementation is always superior.

Benchmark it during development.

The system should record:

```text
processing time
model
memory mode
device
quality mode
```

---

# 13. IMPORTANT — AI MODEL DOWNLOAD

The user currently does NOT have AI models installed.

The application must therefore have a:

# Model Manager

The first time the application is launched:

```text
AI Models

Demucs
Not installed

[ Download ]

Approximate size:
...

Recommended for:
Apple Silicon
```

The application must:

* detect installed models
* download missing model files
* verify checksum
* show progress
* support resume where practical
* store models in application data
* never download models silently
* show license/source information
* allow deleting models
* show disk usage

Do not commit model weights into Git.

---

# 14. EXTERNAL DEPENDENCY MANAGER

Create a diagnostics/dependency screen.

Detect:

```text
FFmpeg
Python
Python environment
Demucs
MLX
Ollama
Ollama model
Audio device
```

Example:

```text
System Readiness

✓ Apple Silicon detected
✓ FFmpeg installed
✓ Python environment ready
✓ Demucs model ready
✓ Audio engine ready
○ Ollama unavailable

Playback:
READY

Preparation:
READY

AI Assistant:
OPTIONAL
```

The application must NOT fail just because Ollama is unavailable.

---

# 15. PYTHON WORKER

Use a dedicated Python environment.

Prefer:

```text
uv
```

for Python environment management if practical.

Create:

```text
audio-worker/
```

Example:

```text
audio-worker/
├── pyproject.toml
├── src/
│   ├── separation/
│   ├── analysis/
│   ├── rendering/
│   ├── audio/
│   └── cli/
└── tests/
```

The Tauri/Rust application communicates with the worker through a local process/API mechanism.

Do NOT expose the worker publicly over the network.

Use:

* stdin/stdout JSON protocol
* local process
* or localhost-only communication

Prefer the simplest robust option.

---

# 16. PREPARATION PIPELINE

Implement the following pipeline:

```text
INPUT
  ↓
Validate file
  ↓
FFmpeg normalization
  ↓
Audio analysis
  ↓
Stem separation
  ↓
Bass adjustment
  ↓
Backing render
  ↓
BPM detection
  ↓
Beat grid
  ↓
Key detection
  ↓
Section detection
  ↓
Cue generation
  ↓
Click configuration
  ↓
Waveform generation
  ↓
Quality control
  ↓
Song Package
```

Do not process multiple songs simultaneously by default.

Use a sequential queue:

```text
Song 1
 ↓
Song 2
 ↓
Song 3
```

This is intentional because the target machine has 24 GB unified memory.

---

# 17. SONG PACKAGE

Every prepared song should have a predictable structure.

Example:

```text
songs/
  goodness-of-god/
    source/
      original.mp3

    stems/
      vocals.wav
      drums.wav
      bass.wav
      other.wav

    rendered/
      backing.wav
      backup.wav

    analysis/
      waveform.json

    song.json
```

Example `song.json`:

```json
{
  "id": "goodness-of-god",
  "title": "Goodness of God",
  "artist": "Bethel Music",
  "bpm": 74,
  "key": "A",
  "timeSignature": "4/4",
  "duration": 287.4,
  "sampleRate": 48000,
  "stems": {
    "vocals": "stems/vocals.wav",
    "drums": "stems/drums.wav",
    "bass": "stems/bass.wav",
    "other": "stems/other.wav"
  },
  "rendered": {
    "backing": "rendered/backing.wav",
    "backup": "rendered/backup.wav"
  },
  "routing": {
    "foh": {
      "backing": true
    },
    "monitor": {
      "click": true,
      "cue": true
    }
  }
}
```

---

# 18. CLICK ENGINE

The user currently does NOT have click audio resources.

Do NOT require downloaded click samples if unnecessary.

Build a **procedural click engine**.

Generate click sounds mathematically.

Required:

```text
Beat click
Accent click
Count-in
```

Parameters:

```text
BPM
Time signature
Accent pattern
Volume
Click sound
```

The click engine should be deterministic.

Example:

```text
4/4

ONE
two
three
four
```

Allow future sound packs.

---

# 19. CLICK SOUND LIBRARY

Although the core click should be procedural, create a resource manager that can optionally download legally licensed click samples.

The application must only use resources with compatible licenses such as:

* CC0
* public domain
* permissive licenses compatible with redistribution

Do NOT download copyrighted commercial samples.

Create:

```text
resources/audio/click/
```

with metadata:

```json
{
  "name": "Default Click",
  "license": "CC0",
  "source": "...",
  "author": "...",
  "downloadedAt": "..."
}
```

The application should have at least one usable default click without requiring internet.

If a legally redistributable sample cannot be safely obtained, use the procedural click instead.

---

# 20. CUE ENGINE

Cue should NOT depend on an AI-generated voice.

The application should initially use macOS system TTS.

Example:

```text
"Intro"
"Verse 1"
"Chorus"
"Bridge"
"Instrumental"
"Outro"
```

Use the macOS built-in voice system where possible.

Generate cue audio during preparation.

For example:

```text
4-beat count-in

"Verse 1"
```

The generated cue should be rendered into an audio file so live playback does not depend on TTS.

---

# 21. CUE RESOURCE MANAGER

Create optional voice-pack support.

Possible future packs:

```text
English
Indonesian
Male
Female
Neutral
```

But MVP should use:

```text
macOS TTS
```

and generate local WAV assets.

Do not require cloud TTS.

---

# 22. SECTION DETECTION

Try to automatically detect:

```text
Intro
Verse
Pre-Chorus
Chorus
Bridge
Instrumental
Outro
```

But AI detection is probabilistic.

Therefore:

```text
Automatic detection
+
Manual correction
```

must be supported.

User can edit:

```text
Intro
Verse 1
Chorus
Verse 2
Chorus
Bridge
Chorus
Outro
```

Markers should be editable.

---

# 23. WORSHIP ARRANGEMENT ENGINE

This is one of the most important differentiators.

A song should have:

```text
Sections

Intro
Verse 1
Chorus
Verse 2
Chorus
Bridge
Chorus
Outro
```

The MD should be able to configure:

```text
Intro
→ Verse 1
→ Chorus
→ Verse 2
→ Chorus
→ Bridge
→ Chorus
→ Outro
```

But also:

```text
Intro
→ Verse 1
→ Chorus
→ Bridge
→ Chorus
→ Outro
```

without editing the underlying audio file.

This means the playback engine should be marker/section aware.

Future feature:

```text
Repeat Chorus
Skip Verse
Jump to Bridge
Extend Worship
```

Do NOT implement all advanced functionality immediately.

Build the architecture so it is possible.

---

# 24. SETLIST

Setlists should be first-class entities.

Example:

```text
Sunday Worship — 12 Oct 2026

01 Goodness of God
02 Build My Life
03 What A Beautiful Name
04 ...
05 ...
```

Allow:

* drag to reorder
* duplicate
* remove
* add song
* save setlist
* rename
* export
* import
* duplicate setlist

---

# 25. SETLIST EDITOR

Layout:

```text
┌──────────────────────────────────────────┐
│ Sunday Worship                  5 Songs  │
├──────────────────────────────────────────┤
│                                          │
│ 01  Goodness of God       A   74 BPM    │
│ 02  Build My Life         D   72 BPM    │
│ 03  What A Beautiful...   E   68 BPM    │
│                                          │
├──────────────────────────────────────────┤
│ [ Add Song ]              [ Save ]       │
└──────────────────────────────────────────┘
```

Each song can have:

```text
Playback Preset
Click
Cue
FOH
Monitor
Starting Section
```

---

# 26. LIVE MODE

LIVE MODE IS THE MOST IMPORTANT UI.

When entering Live Mode:

Hide unnecessary settings.

Show:

```text
┌─────────────────────────────────────────────────┐
│ GOODNESS OF GOD                         01 / 05 │
│ A • 74 BPM                                      │
├─────────────────────────────────────────────────┤
│                                                 │
│                  CHORUS                         │
│                                                 │
│              02:14 / 04:47                      │
│                                                 │
│  Intro ─ Verse ─ Chorus ─ Bridge ─ Chorus      │
│                                                 │
├─────────────────────────────────────────────────┤
│                                                 │
│     ◀ PREV       ▶ PLAY       NEXT ▶           │
│                                                 │
│                   PANIC                         │
└─────────────────────────────────────────────────┘
```

Large controls.

High contrast.

Minimal distractions.

---

# 27. LIVE CONTROLS

Support:

```text
Space
Play/Pause

←
Previous

→
Next

Escape
Panic Stop

N
Next song

P
Previous song
```

Make shortcuts configurable.

Future:

* MIDI foot controller
* Stream Deck
* MIDI notes
* external controller

---

# 28. PANIC STOP

Panic Stop must immediately stop all audio.

It should be visually obvious.

Example:

```text
PANIC
STOP
```

Red/danger styling is acceptable here.

It must NOT require confirmation.

---

# 29. AUDIO OUTPUT ARCHITECTURE

DO NOT hardcode:

```text
Left = FOH
Right = Monitor
```

Internally use logical buses.

Example:

```text
FOH BUS
Monitor BUS
MD BUS
Drummer BUS
```

Then route buses to physical audio outputs.

Example:

```text
FOH BUS
→ Output 1-2

MONITOR BUS
→ Output 3-4
```

The user may still select:

```text
Compatibility Mode:
L/R Split
```

but the internal engine must be bus-based.

---

# 30. DEFAULT ROUTING

Default:

```text
FOH:
Backing

Monitor:
Click
Cue
Optional backing reference
```

Click must NEVER accidentally go to FOH.

Cue must NEVER accidentally go to FOH.

Provide a routing visualization:

```text
BACKING ─────────→ FOH
             └──→ MONITOR

CLICK ───────────→ MONITOR

CUE ─────────────→ MONITOR
```

---

# 31. AUDIO DEVICE SETTINGS

Allow selection of:

```text
Audio Interface
Output Device
Sample Rate
Buffer Size
```

Preferred default:

```text
48 kHz
```

Buffer options:

```text
64
128
256
512
```

Show latency estimate.

Provide:

```text
Test Audio
Test FOH
Test Monitor
Test Click
Test Cue
```

---

# 32. LIVE AUDIO ENGINE

The live engine must be native and deterministic.

Do NOT depend on:

* browser audio
* HTMLAudioElement
* Web Audio API as the primary live engine
* JavaScript timers for musical timing

Use a native audio engine.

Preferred approach:

```text
Rust/Tauri
+
native audio layer
```

Evaluate:

* CPAL
* CoreAudio
* JUCE bridge

For production-quality live playback, prefer CoreAudio/JUCE if necessary.

The final implementation must prioritize:

1. stability
2. low latency
3. sample-accurate timing
4. predictable routing
5. no glitches

---

# 33. WAVEFORM

Generate lightweight waveform peak data during preparation.

Do NOT calculate expensive waveform data every time the song opens.

Store:

```text
analysis/waveform.json
```

Live mode may show a simplified waveform/timeline.

Preparation/editor mode may show detailed waveforms.

---

# 34. AI ASSISTANT

Ollama is OPTIONAL.

The application must work perfectly without it.

Use a provider abstraction:

```text
AIProvider

├── NoAIProvider
├── OllamaProvider
└── FutureCloudProvider
```

Default:

```text
NoAIProvider
```

unless Ollama is detected.

---

# 35. OLLAMA

Support local Ollama.

Target:

```text
8B parameter class model
```

because the development machine has 24 GB RAM.

Prefer a quantized model suitable for 24 GB unified memory.

The user should be able to select:

```text
Ollama Model
```

from Settings.

Never require OpenAI API keys.

Never require Anthropic API keys.

Never send worship audio to the cloud.

---

# 36. WHAT OLLAMA SHOULD DO

Ollama should NOT control the audio engine.

Use it for:

* metadata interpretation
* section naming
* setlist suggestions
* natural language commands
* cue suggestions
* arrangement suggestions
* MD assistant

Example:

User:

> "Untuk lagu ini drummer tidak main, tapi bassist tetap main."

LLM output:

```json
{
  "preset": "NO_DRUMMER",
  "drums": "KEEP",
  "bass": "KEEP",
  "click": false
}
```

But the application must validate the JSON before applying it.

---

# 37. RULES ENGINE

Never allow LLM output to directly execute arbitrary commands.

Pipeline:

```text
User
 ↓
Ollama
 ↓
Structured JSON
 ↓
Schema Validation
 ↓
Rules Engine
 ↓
User Confirmation if necessary
 ↓
Playback Engine
```

The Rules Engine is authoritative.

---

# 38. NO-CLOUD PRINCIPLE

The application should operate without internet after installation.

Internet may only be required for:

* downloading AI models
* downloading optional resources
* application updates
* checking documentation
* optional future cloud AI

Core functionality must work offline.

---

# 39. RESOURCE DOWNLOAD MANAGER

Create a generic resource manager.

Example:

```text
ResourceManager

download()
verifyChecksum()
install()
remove()
getStatus()
```

Resources may include:

```text
AI models
FFmpeg
optional click packs
optional TTS resources
future separation models
```

Every downloaded resource must have:

```text
name
version
source URL
license
checksum
size
installed status
```

Never blindly execute downloaded binaries.

---

# 40. LEGAL / LICENSE SAFETY

When downloading resources:

Only use resources where redistribution/use is legally compatible.

Never scrape random MP3s.

Never download commercial worship songs.

Never bundle copyrighted songs.

The user supplies the source songs.

The application processes them locally.

For bundled resources, keep:

```text
THIRD_PARTY_NOTICES.md
```

with:

```text
Resource
Version
Source
License
Copyright
```

---

# 41. AUDIO FORMAT

Use:

```text
WAV
48 kHz
32-bit float internally
```

for rendered live assets where practical.

Input can be:

```text
MP3
WAV
FLAC
M4A
AIFF
```

Use FFmpeg for decoding/conversion.

Do not repeatedly lossy-compress audio.

---

# 42. FAILSAFE SYSTEM

Every prepared song should have:

```text
Primary backing
+
Backup stereo mix
```

If a stem or routing configuration fails:

```text
Fallback → backup.wav
```

Live playback must not become unusable because an optional stem is missing.

Show:

```text
READY
WARNING
ERROR
```

before entering live mode.

---

# 43. PREFLIGHT CHECK

Before Live Mode:

```text
LIVE PREFLIGHT

✓ All songs prepared
✓ Audio files exist
✓ Sample rate = 48 kHz
✓ Audio device connected
✓ FOH routing OK
✓ Monitor routing OK
✓ Click available
✓ Cue available
✓ Backup tracks available
✓ No missing assets

READY FOR LIVE
```

If something fails:

```text
NOT READY
```

and explain why.

---

# 44. DATABASE

Use SQLite.

Entities:

```text
Song
SongStem
SongSection
Setlist
SetlistItem
Preset
AudioDevice
RoutingProfile
Resource
PreparationJob
```

Use migrations.

Keep database schema versioned.

Do not store large WAV files inside SQLite.

Store paths.

---

# 45. FILE STORAGE

Use the operating system's application data directory.

Do not assume the working directory.

Example conceptual structure:

```text
Application Data/
  WorshipPlaybackEngine/
    database.sqlite

    songs/
    setlists/

    models/
    resources/

    cache/
    logs/
```

Allow user to choose a custom library location in Settings.

---

# 46. BACKUP

Provide:

```text
Export Library
Import Library
```

At minimum support exporting:

```text
song metadata
setlists
presets
routing profiles
```

Optionally package audio files.

---

# 47. LOGGING

Create structured logs.

Categories:

```text
Audio
Preparation
AI
Routing
Playback
System
Error
```

Never flood the user interface with technical logs.

Have:

```text
Diagnostics → Logs
```

for troubleshooting.

---

# 48. ERROR HANDLING

Never show raw stack traces to normal users.

Instead:

```text
We couldn't prepare this song.

Reason:
The separation engine stopped unexpectedly.

[ Retry ]
[ Use Backup ]
[ View Details ]
```

Technical details can be expanded.

---

# 49. PERFORMANCE

Target MacBook Air M4 24 GB.

Important:

Do NOT run multiple large AI models simultaneously.

Preparation should use a queue.

Example:

```text
Preparing Song 1...
██████████████░░░░ 72%

Next:
Song 2
Song 3
```

Keep memory under control.

Release model resources when appropriate.

---

# 50. TESTING

Create automated tests for:

## Rust

* routing
* setlist ordering
* song loading
* preflight
* file validation
* JSON schema
* playback state machine

## TypeScript

* UI components
* setlist
* song editor
* preset logic

## Python

* audio processing
* separation
* rendering
* BPM
* cue generation
* waveform generation

---

# 51. PLAYBACK STATE MACHINE

Create an explicit state machine.

Example:

```text
STOPPED
READY
PLAYING
PAUSED
LOADING
NEXTING
ERROR
PANIC
```

Do not implement playback as scattered booleans.

---

# 52. SETLIST STATE MACHINE

Example:

```text
SETLIST_READY
SONG_LOADING
SONG_READY
SONG_PLAYING
SONG_COMPLETED
TRANSITIONING
```

This will make dynamic arrangement safer.

---

# 53. FUTURE MIDI

Design interfaces now but do not overbuild.

Future support:

```text
MIDI Clock
MIDI Input
MIDI Foot Controller
MIDI Program Change
```

Possible commands:

```text
Next
Previous
Play
Stop
Jump Section
```

---

# 54. FUTURE ABLETON LINK

Create an abstraction:

```text
SyncProvider
```

Future implementation:

```text
AbletonLinkProvider
MidiClockProvider
InternalClockProvider
```

Do not make Ableton Link mandatory.

---

# 55. REHEARSAL MODE

Separate from Live Mode.

Rehearsal mode can have:

* waveform
* seek
* loop section
* stem solo/mute
* BPM adjustment
* click volume
* cue preview
* section jump
* detailed meters

Live Mode should be intentionally simplified.

---

# 56. SONG EDITOR

Song editor should allow:

```text
Title
Artist
BPM
Key
Time Signature

Sections
Markers

Vocals
Drums
Bass
Other

FOH
Monitor

Click
Cue
```

Include preview playback.

---

# 57. METERING

Provide basic meters:

```text
FOH L
FOH R

MONITOR L
MONITOR R
```

Avoid expensive spectrum analyzers in MVP.

Simple peak/RMS meters are enough.

---

# 58. DESIGN SYSTEM

Use a professional dark interface.

Design characteristics:

* dark charcoal background
* subtle surfaces
* clear hierarchy
* restrained accent color
* rounded but not overly playful
* large typography for live mode
* compact controls in preparation mode
* consistent spacing
* keyboard-first interaction

Do not make it look like a SaaS dashboard.

Do not use excessive gradients.

Do not use unnecessary animations.

---

# 59. RESPONSIVE LAYOUT

Although this is a desktop app, support different Mac window sizes.

Minimum useful window:

```text
1100 × 700
```

Recommended:

```text
1440 × 900
```

Live mode should scale elegantly.

---

# 60. ACCESSIBILITY

Support:

* keyboard navigation
* visible focus states
* sufficient contrast
* tooltips
* readable font sizes
* configurable UI scale

---

# 61. PROJECT STRUCTURE

Start with a clean monorepo-like structure:

```text
worship-playback-engine/
│
├── apps/
│   └── desktop/
│       ├── src/
│       ├── src-tauri/
│       └── package.json
│
├── audio-worker/
│   ├── pyproject.toml
│   ├── src/
│   └── tests/
│
├── shared/
│   ├── schemas/
│   └── types/
│
├── resources/
│
├── docs/
│
├── scripts/
│
├── tests/
│
├── THIRD_PARTY_NOTICES.md
├── README.md
└── LICENSE
```

Adjust the structure if Tauri best practices require a better arrangement.

Do not force the structure blindly.

---

# 62. IPC CONTRACT

Define strongly typed IPC between:

```text
React
 ↕
Tauri/Rust
 ↕
Python worker
```

Do not pass arbitrary unvalidated objects.

Use schemas.

Prefer JSON-compatible structured commands.

Example:

```json
{
  "type": "prepare_song",
  "songId": "...",
  "input": "...",
  "preset": "FULL_BAND"
}
```

---

# 63. EVENT SYSTEM

Create events such as:

```text
preparation.started
preparation.progress
preparation.completed
preparation.failed

playback.started
playback.paused
playback.stopped
playback.position
playback.sectionChanged
playback.songChanged

audio.deviceChanged
audio.error
```

This allows the UI to remain decoupled from the audio engine.

---

# 64. MVP SCOPE

DO NOT attempt to build everything immediately.

Phase 1:

```text
Tauri app
React UI
Rust backend
SQLite
Import song
Play song
Song library
Basic setlist
Basic live mode
```

Phase 2:

```text
Python worker
FFmpeg
Demucs
Stem generation
Backing generation
```

Phase 3:

```text
BPM
Key
Beat grid
Sections
Click
Cue
```

Phase 4:

```text
FOH/Monitor buses
Audio device routing
Preflight
Backup
```

Phase 5:

```text
Ollama
AI assistant
Natural language commands
```

Phase 6:

```text
MIDI
Ableton Link
Advanced arrangement
```

---

# 65. IMPORTANT DEVELOPMENT STRATEGY

Do NOT write thousands of lines of code immediately.

Work incrementally.

After each major phase:

1. Build
2. Run
3. Test
4. Fix errors
5. Commit
6. Continue

Create Git commits such as:

```text
feat: initialize tauri desktop app
feat: add song library
feat: add sqlite persistence
feat: add basic playback engine
feat: add setlist
feat: add live mode
feat: add python audio worker
feat: add demucs preparation
feat: add click engine
feat: add cue engine
feat: add routing
feat: add preflight
feat: add ollama provider
```

---

# 66. FIRST TASK FOR CURSOR

Before writing significant application code:

## STEP 1 — AUDIT ENVIRONMENT

Inspect:

```text
macOS version
Apple Silicon architecture
Node version
pnpm/npm
Rust
Cargo
Python
uv
FFmpeg
Ollama
Xcode Command Line Tools
```

Do not install unnecessary software.

Report what exists.

---

# 67. INSTALLATION

If dependencies are missing, create a setup script.

Use Homebrew where appropriate.

Possible dependencies:

```text
Node
pnpm
Rust
Python
uv
FFmpeg
```

Do not install anything without explaining why.

Do not install cloud services.

---

# 68. RESOURCE DISCOVERY

For required open-source components:

Search official repositories/documentation first.

Prioritize:

* official project repository
* official package registry
* reputable maintained fork
* Apple Silicon compatible implementation

Avoid random GitHub repositories unless necessary.

For AI separation, investigate:

* Demucs
* Apple Silicon MLX implementations
* suitable maintained alternatives

For audio:

* FFmpeg
* CoreAudio
* CPAL/JUCE depending on final architecture

Document every dependency and license.

---

# 69. DO NOT ASSUME DEMUCS IS FINAL

Create a benchmark script.

Test at least:

```text
Demucs standard
Demucs Apple Silicon / MLX implementation if compatible
```

on a representative test song.

Measure:

```text
processing time
RAM usage
CPU/GPU utilization
output quality
```

Do not make architectural decisions based only on README claims.

---

# 70. TEST AUDIO

Because the user currently has no dedicated click/cue resources, create synthetic test audio.

The development system should be able to generate:

```text
test_click.wav
test_cue.wav
test_tone.wav
test_backing.wav
```

using code.

Do not use copyrighted worship music as bundled test data.

Create simple generated test signals.

---

# 71. SAMPLE TEST SONG

Do not download copyrighted commercial worship songs automatically.

Instead:

* generate synthetic audio
* optionally allow the developer to manually import their own song
* use public-domain / appropriately licensed test audio when available

The user owns/provides the actual worship songs.

---

# 72. FIRST UI PROTOTYPE

Before AI integration, build this flow:

```text
Dashboard
 ↓
Songs
 ↓
Import Song
 ↓
Song Detail
 ↓
Set Preparation
 ↓
Generate
 ↓
Song Ready
 ↓
Add to Setlist
 ↓
Live Mode
 ↓
Play
```

The application must already feel coherent before AI is introduced.

---

# 73. FIRST ACCEPTANCE TEST

At the end of the first MVP:

I should be able to:

1. Launch the Mac application.
2. Import an MP3/WAV.
3. See the song in the library.
4. Add it to a setlist.
5. Open Live Mode.
6. Press Play.
7. Hear the song.
8. Press Next.
9. Press Panic.
10. Restart playback.

No AI required for this stage.

---

# 74. SECOND ACCEPTANCE TEST

After audio preparation:

I should be able to:

1. Import a song.
2. Click Prepare.
3. Choose FULL BAND.
4. Run local stem separation.
5. Generate backing.wav.
6. Generate click.
7. Generate cue.
8. Detect BPM.
9. Detect key.
10. Detect sections.
11. Save the prepared song.
12. Add it to a setlist.
13. Play it live.

---

# 75. THIRD ACCEPTANCE TEST

Audio routing:

```text
FOH → backing

Monitor → backing + click + cue
```

Verify that:

```text
click is NOT present in FOH
cue is NOT present in FOH
```

This test is mandatory.

---

# 76. OLLAMA ACCEPTANCE TEST

If Ollama is installed:

User can write:

> "Untuk lagu ini drummer tidak main, bassist main."

The system converts it into validated structured settings.

But if Ollama is OFF:

The rest of the application continues working normally.

---

# 77. SECURITY

Never:

* execute arbitrary shell commands from LLM output
* download arbitrary files from LLM output
* allow LLM to control the filesystem freely
* allow LLM to modify application configuration without validation

All AI commands must pass through strict schemas.

---

# 78. DATA PRIVACY

The app should be explicitly:

> Local-first.

Audio files remain on the user's Mac.

No automatic cloud upload.

No analytics by default.

No telemetry by default.

No worship audio leaves the machine.

---

# 79. README

Create a professional README containing:

```text
What is Worship Playback Engine?
Features
Architecture
Installation
Development
AI Models
Audio Preparation
Live Mode
Resource Management
Licenses
Privacy
Roadmap
```

---

# 80. ARCHITECTURE DOCUMENT

Create:

```text
docs/ARCHITECTURE.md
```

Document:

```text
Frontend
Tauri
Rust
Python
Audio Engine
AI Engine
Database
File Storage
IPC
Routing
Playback
Preparation
```

Create diagrams using Mermaid where helpful.

---

# 81. IMPORTANT PRODUCT PHILOSOPHY

Keep this sentence visible in the architecture documentation:

> **AI prepares. Rules decide. Audio Engine performs.**

And:

> **If AI disappears, worship playback must continue.**

---

# 82. DO NOT OVERENGINEER

Avoid prematurely implementing:

* cloud accounts
* authentication
* church multi-tenancy
* subscriptions
* online sync
* remote collaboration
* complicated plugin systems
* VST hosting
* full DAW editing
* AI chat everywhere

This is a local desktop application first.

---

# 83. DEFINITION OF DONE FOR MVP

MVP is considered successful when:

```text
✓ Native Mac application
✓ Tauri 2
✓ React + TypeScript
✓ Rust backend
✓ SQLite
✓ Local audio playback
✓ Song library
✓ Setlists
✓ Live Mode
✓ Panic Stop
✓ Keyboard shortcuts
✓ Audio device selection
✓ FOH / Monitor logical buses
✓ Local Python worker
✓ FFmpeg
✓ Local stem separation
✓ Backing generation
✓ Procedural click
✓ Local cue generation
✓ BPM detection
✓ Basic section markers
✓ Preflight
✓ Backup playback
✓ Offline operation
✓ No API keys
✓ No mandatory cloud service
✓ No mandatory Ollama
```

---

# 84. CURSOR BEHAVIOR

You are operating as an autonomous senior engineer inside Cursor.

Before implementing a feature:

1. Inspect the existing project.
2. Understand current architecture.
3. Do not overwrite working code unnecessarily.
4. Prefer small, testable changes.
5. Explain architectural decisions briefly.
6. Implement.
7. Run tests.
8. Run type checking.
9. Run linting.
10. Build the Tauri application.
11. Fix errors.
12. Report what changed.

Do not merely generate code without running it.

---

# 85. IMPORTANT: WHEN YOU NEED EXTERNAL RESOURCES

If a dependency, model, or resource is required:

1. Search for the official source.
2. Verify it is compatible with macOS Apple Silicon.
3. Verify license.
4. Download/install it.
5. Verify checksum if available.
6. Record source/version/license.
7. Add documentation.
8. Do not commit large model files to Git.

If a resource is optional, do not make it a hard dependency.

---

# 86. START NOW

Begin by performing an environment audit.

Then:

### Phase 0

Create:

```text
docs/ARCHITECTURE.md
docs/ROADMAP.md
docs/DEPENDENCIES.md
THIRD_PARTY_NOTICES.md
```

Then initialize the Tauri 2 application.

Do NOT implement the complete product in one step.

After the audit, propose the smallest first implementation milestone.

Then implement that milestone completely.

Run it.

Test it.

Only then proceed.

The ultimate goal is a reliable, beautiful, local-first Mac application that a church MD can trust on Sunday morning.

Remember:

> **This is not a DAW.**
>
> **This is not Moises.**
>
> **This is not an AI demo.**
>
> **This is a Worship Playback Engine.**
>
> **From Song to Sunday. Automatically.**
