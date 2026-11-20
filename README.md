# Quick Note

A tiny GNOME popup for jotting a note into your [Obsidian](https://obsidian.md) vault
from anywhere. Press a global shortcut, type, hit Enter — the note is written
straight into the vault using your [QuickAdd](https://github.com/chhoumann/quickadd)
**Capture** choices. Obsidian does not need to be running.

Built with Rust, GTK 4 and libadwaita.

## Features

- **Uses your existing QuickAdd setup.** Reads `.obsidian/plugins/quickadd/data.json`
  and offers every Capture choice (including ones nested in Multi choices), with the
  same target paths, format strings and placement rules.
- **Fast and light.** Uses GTK's cairo renderer by default, so no Vulkan/Mesa/LLVM
  stack is loaded: about 68 MB RSS / 23 MB PSS while open, nothing in the background
  when closed, and the window closes about 120 ms after you press Enter.
- **Compact window** that grows with your text and shows where the note will go
  (`→ folder/file.md`) as you type.
- **A colour per choice**: a glowing line along the top edge, the choice pill, the
  file name and the caret all take the choice's colour. Follows the system light/dark
  style.
- **Keyboard first**: Enter saves, Shift+Enter adds a line, Ctrl+1–9 switches choice,
  Esc closes.
- **Native input**: works with input methods such as Fcitx5 (e.g. Vietnamese
  with Unikey), clipboard paste and accessibility, because it is a plain GTK text view.

## Requirements

- GNOME on Wayland or X11 (the global shortcut is registered as a GNOME custom
  keyboard shortcut)
- GTK ≥ 4.12 and libadwaita ≥ 1.5
- Rust ≥ 1.85 (edition 2024) to build

On Ubuntu/Debian:

```sh
sudo apt install build-essential libgtk-4-dev libadwaita-1-dev
```

## Install

```sh
git clone https://github.com/tecix/quicknote.git
cd quicknote
./install.sh
```

`install.sh` builds a release binary and installs it to `~/.local/bin/quicknote`
together with a desktop entry. Make sure `~/.local/bin` is on your `PATH`.

## Usage

1. Run `quicknote --settings` (or open **Quick Note → Settings** from the app grid).
2. Pick your vault: detected Obsidian vaults are listed, or browse to a folder.
3. Choose a default Capture choice.
4. Under **Global shortcut**, press **Change…** and press the key combination you
   want. It is saved as a GNOME custom shortcut that runs `quicknote`.

Press the shortcut anywhere to open the capture window.

| Key | Action |
| --- | --- |
| Enter | Save the note and close |
| Shift+Enter | New line |
| Ctrl+1 … Ctrl+9 | Switch to the 1st … 9th Capture choice |
| Ctrl+, | Open settings |
| Esc | Close without saving (the text is discarded) |

Pressing the shortcut again while the window is open brings it back to the front
with your text intact.

If a save fails (for example the target file does not exist and the choice does not
create it), the window stays open with your text and shows the error.

Settings are stored in `~/.config/quicknote/config.json`. The **Window opacity**
setting makes the window background translucent.

## QuickAdd support

Supported for Capture choices:

- **Capture to** a file path, including format syntax in the path
- **Format** with `{{VALUE}}`, `{{VALUE:name}}`, `{{DATE}}`, `{{DATE:format}}`,
  `{{DATE+n}}`, `{{DATE:format+n}}`, `{{TIME}}`, `{{GLOBAL_VAR::name}}` and `\n`.
  Date formats follow moment.js tokens (`YYYY`, `MM`, `DD`, `ddd`, `GGGG`, `WW`,
  `[literal]`, …) in the English locale.
- **Task** (prefixes `- [ ] `), **prepend**
- **Insert after** a heading, with *insert at end of section*, *consider
  subsections* and *create heading if not found* (top or bottom)
- **Create file if it doesn't exist**, optionally from a template

Not supported:

- *Capture to active file* (needs Obsidian running)
- Capture targets that are folders, and `.canvas` files
- Template and Macro choices

Writes are atomic: the note is written to a temporary file and renamed into place.

## Performance notes

GTK 4 defaults to the Vulkan renderer, which loads Mesa and libLLVM and roughly
doubles memory for a window this small. Quick Note sets `GSK_RENDERER=cairo` unless
you set `GSK_RENDERER` yourself.

[LEARNINGS.md](LEARNINGS.md) records the measurements and the approaches that were
tried and rejected (a resident daemon, a custom Wayland client).

## Limitations

- GNOME on Wayland does not let apps keep a window always on top. Use
  Alt+Space → *Always on Top* if you need it.
- The global shortcut relies on GNOME's custom keyboard shortcuts; other desktops
  can bind `quicknote` to a key themselves.

## Project layout

```
core/   QuickAdd format syntax, placement rules and vault file handling (no GUI)
app/    GTK 4 / libadwaita application: capture window, settings, shortcut
data/   Desktop entry
```

Run the tests with `cargo test`.

## License

[MIT](LICENSE)
