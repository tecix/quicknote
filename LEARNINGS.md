# Performance & Architecture Learnings

Notes from experimenting with ways to speed up launching the Quick Note capture window on GNOME Wayland (Ubuntu, GTK 4.22, libadwaita 1.9, Fcitx5 + Unikey).

## 1. Why normal GTK4 cold start feels slow
- The `quicknote` binary dynamically links **129 shared libraries** (GTK4, libadwaita, Pango, Cairo, Fontconfig, GStreamer, Glycin, Vulkan/Mesa/LLVM).
- Observed startup CPU cost:
  - Process + GTK4/Adwaita init (`--daemon`, before opening a window): **~400 ms CPU**, ~13 MB cgroup RAM.
  - Opening the first window triggers Vulkan/Mesa + `libLLVM.so.21.1` initialization: **~1.5 s CPU** total.
- Minor code-level overhead inside the app (still safe to optimize later without changing the toolkit):
  - `capture.rs` reads & parses QuickAdd's `data.json` twice before presenting (once in `list_capture_choices`, once in `preview_target`) and re-reads it on every keystroke (`buffer.connect_changed`).
  - `core/src/format.rs` recompiles 5 case-insensitive `regex::Regex` instances on every `format_text()` call instead of using `std::sync::LazyLock`.

## 2. Experiment A — Resident GTK4 daemon + `gapplication launch`
- **What worked:**
  - Running `quicknote --daemon` via `systemd --user` + D-Bus activation and binding the GNOME shortcut to `gapplication launch dev.tecix.QuickNote` (only 15 shared libs, preserves Wayland activation tokens) made window opening nearly instant.
- **Why it was rejected:**
  - Once a window had been opened, the resident daemon sat at **~144 MB RSS** (**100.9 MB** systemd cgroup, **59.3 MB** PSS, **42.3 MB** private dirty):
    - **54.1 MB RSS** from Mesa Vulkan + `libLLVM.so.21.1` (42.8 MB alone).
    - **35.1 MB** heap/anon (Vulkan/LLVM shader compiler state + GTK4 caches).
    - **55.1 MB RSS** (12.3 MB PSS, 3.4 MB private dirty) from GTK4/libadwaita/Fontconfig/GStreamer/Glycin shared libraries.
  - Even without Vulkan/GL (`GSK_RENDERER=cairo` + `GDK_DISABLE=vulkan,gl`), a resident GTK4 + libadwaita daemon cannot stay under **10 MB** RSS because the toolkit and fontconfig mappings alone exceed that.

## 3. Experiment B — Custom native Wayland + PangoCairo capture binary (no daemon)
- **What worked:**
  - Splitting capture into a standalone `smithay-client-toolkit` + PangoCairo binary dropped shared libraries from **129 → 42** (no GTK4, libadwaita, Vulkan, LLVM, Mesa, GStreamer) with **0 MB** background memory and very fast cold launch.
- **Why it was reverted:**
  - Re-implementing a Wayland text widget from scratch breaks subtle desktop integrations that GTK4 handles reliably out of the box:
    - **Clipboard paste (`Ctrl+V`) failed**: Wayland `wl_data_device` selection offers depend on compositor focus/serial timing and non-blocking pipe dispatch; blocking `read_to_string` inside the Wayland event loop deadlocks or misses offers from other clients.
    - **Vietnamese typing (Fcitx5 + Unikey) failed**: on GNOME Wayland, Mutter does not route `zwp_text_input_v3` to Fcitx5, and talking to Fcitx5's `org.freedesktop.portal.Fcitx` D-Bus API directly has subtle capability/focus/key-event differences (e.g., `libim-fcitx5.so` in GTK4 handles `SetSurroundingText`, client-side UI, keycode/keysym translation, and async D-Bus signals).
  - Keeping GTK4 (`GtkTextView` + `GTK_IM_MODULE=fcitx`) guarantees working clipboard paste, IME/Vietnamese input, accessibility, cursor/selection UX, and native Adwaita look & feel.

## 4. Safe, low-risk ideas to consider next (staying on GTK4, no daemon)
If revisiting launch speed later without giving up GTK4 or keeping a permanent background process:
1. **Disable GPU/Vulkan init for the tiny text popup**: test `GSK_RENDERER=cairo` + `GDK_DISABLE=vulkan,gl` on cold launch — avoids loading the 54 MB Mesa/Vulkan/LLVM stack and ~1.5 s GPU init while keeping **0 MB** background RAM and full GTK4 paste + Fcitx5 Vietnamese support.
2. **Core micro-optimizations**: hoist the 5 regexes in `core/src/format.rs` into `LazyLock<Regex>` and load QuickAdd's `data.json` once per launch instead of twice + every keystroke.
