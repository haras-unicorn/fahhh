# fahhh

A software soundboard that runs in the background and plays sounds when you
press configurable key combinations.

## Installation

`fahhh` is distributed as a [Nix flake](https://github.com/haras-unicorn/fahhh).

If you are using Nix, you can run it directly:

```bash
nix run github:haras-unicorn/fahhh
```

Alternatively, download the standalone binary from the
[Releases page](https://github.com/haras-unicorn/fahhh/releases) for Linux
(Wayland, X11 not yet supported) and Windows (MacOS not supported because I
can't test for it because I don't have the hardware).

## Usage

Run the standalone binary and the `fahhh` window will appear with the systray
running in the background.

## Features

- **Configurable**: You can configure any key or combination of keys to press to
  play sounds
- **Blazingly fast**: Written in rust and loads your sound files into memory for
  minimal latency in playback

## Security & Permissions

- **Linux (Wayland)**: Uses `xdg-desktop-portal` which means you will see a
  prompt to allow global shortcut access. Denial means hotkeys won't work.
- **Windows**: Uses `RegisterHotKey` which means no prompt but hotkeys may fail
  silently if another app uses the same hotkey.
