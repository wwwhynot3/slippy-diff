# Release Packaging And Desktop Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Windows executable icon embedding and ship a Linux desktop-integration bundle alongside the existing raw binary and AppImage release assets.

**Architecture:** Keep runtime UI icon behavior unchanged and extend release packaging around it. Centralize Linux desktop metadata under `packaging/linux`, generate Windows executable resources during build, and make release workflow tests define the packaging contract before editing workflow logic.

**Tech Stack:** Rust, Cargo build scripts, GitHub Actions, shell install/uninstall scripts

---

### Task 1: Lock The Release Contract In Tests

**Files:**
- Modify: `tests/release_workflow.rs`

- [ ] **Step 1: Add failing workflow assertions for Windows icon embedding and Linux bundle outputs**

Add assertions that expect:

- the Windows job to run a build step that can embed an icon resource
- the Linux binary job to also produce `-bundle.tar.gz` assets
- the AppImage job to reuse checked-in Linux desktop metadata instead of an inline here-doc

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test release_workflow -- --nocapture`
Expected: FAIL because the workflow still uploads only the raw Linux binary, still writes inline AppImage metadata, and has no Windows icon-resource setup.

### Task 2: Add Windows Executable Icon Resources

**Files:**
- Modify: `Cargo.toml`
- Create: `build.rs`
- Create: `assets/icons/slippy.ico`
- Modify: `assets/icons/README.md`

- [ ] **Step 1: Add the failing Windows packaging expectations to the codebase context**

Use the Task 1 failing test as the guardrail; do not change production files until that test is red for the expected workflow gaps.

- [ ] **Step 2: Add minimal Windows resource embedding support**

Implement a `build.rs` that:

- no-ops on non-Windows targets
- points `winresource` at `assets/icons/slippy.ico`
- compiles the icon resource into the Windows executable

Add the matching Cargo build dependency.

- [ ] **Step 3: Add the `.ico` asset and document how it relates to the existing SVG/PNG icon set**

Commit `assets/icons/slippy.ico` and update the icons README so contributors know the SVG remains the source asset and the `.ico` is the Windows packaging derivative.

- [ ] **Step 4: Re-run the focused test**

Run: `cargo test release_workflow -- --nocapture`
Expected: still FAIL, but only on Linux bundle and AppImage metadata assertions.

### Task 3: Add Linux Desktop Bundle Assets And Scripts

**Files:**
- Create: `packaging/linux/install-linux.sh`
- Create: `packaging/linux/install-release.sh`
- Create: `packaging/linux/uninstall-linux.sh`
- Create: `packaging/linux/README.md`

- [ ] **Step 1: Write user-scoped install and uninstall scripts**

`install-linux.sh` should:

- resolve its own bundle directory
- create `~/.local/bin`, `~/.local/share/applications`, and the needed `~/.local/share/icons/hicolor/.../apps` directories
- install `slippy`
- install `~/.local/bin/slippy-uninstall`
- copy the checked-in desktop file to the user applications directory
- rewrite only the installed desktop file so `Exec=` points to `~/.local/bin/slippy`
- copy all shipped icon sizes
- print success plus uninstall instructions

`install-release.sh` should:

- download the latest release bundle by default
- allow `SLIPPY_VERSION`, `SLIPPY_ARCH`, and `SLIPPY_BACKEND` overrides
- unpack the matching bundle into a temporary directory
- invoke that bundle's `install-linux.sh`

`uninstall-linux.sh` should:

- remove only the installed Slippy binary, desktop file, and icon files
- tolerate missing files
- print a completion message

- [ ] **Step 2: Add bundle-local documentation**

Create a short `packaging/linux/README.md` that explains:

- how to run `install-linux.sh`
- how to run `~/.local/bin/slippy-uninstall`
- that the raw binary alone does not create a launcher entry

### Task 4: Update Release Workflow To Build And Upload Bundle Assets

**Files:**
- Modify: `.github/workflows/release-please.yml`

- [ ] **Step 1: Reuse checked-in Linux metadata in the AppImage job**

Replace the inline `cat <<DESKTOP` block with copies from `packaging/linux/` and normalize the AppImage-local filenames so `linuxdeploy` still finds `slippy.desktop` and `slippy.png`.

- [ ] **Step 2: Extend the Linux binary job to also build a bundle tarball**

After building the raw binary:

- stage a temporary bundle directory
- copy in `slippy`, `install-linux.sh`, `install-release.sh`, `uninstall-linux.sh`, bundle `README.md`, the desktop file, and all icon sizes
- create `slippy-v${RELEASE_VERSION}-linux-${{ matrix.asset_arch }}-${{ matrix.backend }}-bundle.tar.gz`
- upload both the raw binary and the bundle tarball

- [ ] **Step 3: Ensure raw binary asset naming remains unchanged**

Keep the existing four direct Linux binary asset names intact.

- [ ] **Step 4: Re-run the focused workflow test**

Run: `cargo test release_workflow -- --nocapture`
Expected: PASS.

### Task 5: Document User-Facing Install Paths

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update Linux release instructions**

Document:

- raw binary usage
- desktop-integrated install via the bundle
- AppImage usage

Include a one-command `curl | bash` installer example plus the matching uninstall command.

- [ ] **Step 2: Keep the README wording aligned with actual installed paths**

Confirm the documented locations match the shell scripts exactly.

### Task 6: Final Verification

**Files:**
- No code changes expected

- [ ] **Step 1: Run formatting if needed**

Run: `cargo fmt`
Expected: exit 0

- [ ] **Step 2: Run the focused release workflow test**

Run: `cargo test release_workflow -- --nocapture`
Expected: PASS

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: PASS
