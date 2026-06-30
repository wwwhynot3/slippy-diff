# Slippy Release Packaging And Desktop Integration Design

## Decision

Extend Slippy's release packaging in two places:

- Embed the application icon into Windows release executables.
- Keep publishing the standalone Linux binary, and additionally publish a versioned Linux desktop-integration bundle that contains the binary, icons, `.desktop` metadata, and user-scoped install/uninstall scripts.

Keep the existing AppImage flow and align it to the same desktop metadata and icon assets.

## Goals

- Windows users should see the Slippy app icon in Explorer, taskbar pins, and shortcuts created from the released `.exe`.
- Linux users who prefer the raw binary should have an official, version-matched path to install launcher metadata and icons without needing root.
- Linux release assets should still support the simplest download path for advanced users who only want the raw executable.
- Packaging metadata should stay centralized so Linux AppImage and Linux desktop bundle outputs do not drift.

## Non-Goals

- Do not introduce a Linux system-wide package format such as `.deb` or `.rpm`.
- Do not remove the existing AppImage output.
- Do not add persistent runtime dependencies just to load icons.
- Do not change the current in-app FLTK window icon behavior.

## Windows Packaging

- Generate a Windows `.ico` asset from the existing Slippy icon source set.
- Embed that `.ico` into the release executable during Windows builds.
- The embedded icon must ship in both `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` release outputs.
- The release artifact name stays `slippy-v<version>-windows-<arch>.exe`.

## Linux Release Assets

Keep the current standalone Linux binary assets exactly as direct executable downloads:

- `slippy-v<version>-linux-amd64-x11`
- `slippy-v<version>-linux-amd64-wayland`
- `slippy-v<version>-linux-arm64-x11`
- `slippy-v<version>-linux-arm64-wayland`

Add parallel bundle assets for desktop integration:

- `slippy-v<version>-linux-amd64-x11-bundle.tar.gz`
- `slippy-v<version>-linux-amd64-wayland-bundle.tar.gz`
- `slippy-v<version>-linux-arm64-x11-bundle.tar.gz`
- `slippy-v<version>-linux-arm64-wayland-bundle.tar.gz`

Each bundle contains:

- `slippy`
- `install-linux.sh`
- `install-release.sh`
- `uninstall-linux.sh`
- `share/applications/dev.wwwhynot3.slippy.desktop`
- `share/icons/hicolor/<size>x<size>/apps/dev.wwwhynot3.slippy.png` for all shipped PNG sizes
- a short bundle `README.md` describing install and uninstall commands

## Linux Install Behavior

The Linux install script is user-scoped by default and does not require root.

Install locations:

- binary: `~/.local/bin/slippy`
- desktop file: `~/.local/share/applications/dev.wwwhynot3.slippy.desktop`
- icons: `~/.local/share/icons/hicolor/<size>x<size>/apps/dev.wwwhynot3.slippy.png`

Install behavior requirements:

- create any missing target directories
- install the binary with executable permissions
- copy the desktop file and icons from the unpacked bundle
- ensure the desktop file launches the installed binary from `~/.local/bin/slippy`
- prefer a desktop-file `Exec` value that is stable after installation rather than one that depends on the unpack directory
- print a short success message that includes the uninstall command
- install a stable user-scoped uninstaller entry at `~/.local/bin/slippy-uninstall`

## Linux Uninstall Behavior

The uninstall script removes only the files installed by the Slippy user-scoped installer:

- `~/.local/bin/slippy`
- `~/.local/bin/slippy-uninstall`
- `~/.local/share/applications/dev.wwwhynot3.slippy.desktop`
- all Slippy icon files written under `~/.local/share/icons/hicolor/.../apps/`

Uninstall requirements:

- succeed even if some files are already missing
- avoid deleting non-Slippy files
- print a short completion message

## Metadata Reuse

The authoritative Linux desktop metadata remains under `packaging/linux/`.

Requirements:

- AppImage assembly should reuse the checked-in `.desktop` file and icon assets instead of recreating equivalent metadata inline in the workflow
- Linux bundle creation should reuse the same checked-in `.desktop` file and icon assets
- any install-time `Exec` adjustment should happen in the installer or in a generated copy, not by mutating the checked-in source asset in place

## Documentation

Update the top-level README to describe three Linux paths:

- raw binary download
- desktop-integrated install via the bundle
- AppImage download

README requirements:

- include a one-command example that downloads the versioned Linux bundle, extracts it, and runs `install-linux.sh`
- include a one-command `curl | bash` installer example
- include the matching uninstall command at `~/.local/bin/slippy-uninstall`
- explain that the raw binary alone does not register a launcher icon or menu entry

## Verification

- verify the Rust project still builds on the host platform
- verify tests that cover release workflow expectations pass
- add or update release-workflow tests to assert:
  - Windows release outputs still exist and now expect icon embedding support in the build configuration
  - Linux raw binary assets still upload unchanged
  - Linux bundle assets are built and uploaded
  - AppImage assembly uses checked-in desktop metadata assets rather than an inline here-doc
