const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release-please.yml");
const CARGO_TOML: &str = include_str!("../Cargo.toml");
const BUILD_RS: &str = include_str!("../build.rs");

fn job_section<'a>(workflow: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = workflow
        .find(start)
        .unwrap_or_else(|| panic!("missing workflow job section: {start}"));
    let tail = &workflow[start_index..];
    let end_index = tail
        .find(end)
        .unwrap_or_else(|| panic!("missing workflow job boundary: {end}"));

    &tail[..end_index]
}

#[test]
fn linux_appimage_release_matrix_builds_x11_and_wayland_assets() {
    let job = job_section(
        RELEASE_WORKFLOW,
        "  linux-appimages:",
        "  windows-binaries:",
    );

    assert!(job.contains("name: Linux ${{ matrix.asset_arch }} ${{ matrix.backend }} AppImage"));
    assert_eq!(job.matches("asset_arch: amd64").count(), 2);
    assert_eq!(job.matches("asset_arch: arm64").count(), 2);
    assert_eq!(job.matches("backend: x11").count(), 2);
    assert_eq!(job.matches("backend: wayland").count(), 2);
    assert_eq!(job.matches("cargo_features: \"\"").count(), 2);
    assert_eq!(
        job.matches("cargo_features: \"--features wayland\"")
            .count(),
        2
    );

    assert!(job.contains("libwayland-dev"));
    assert!(job.contains("libxkbcommon-dev"));
    assert!(job.contains("libdbus-1-dev"));
    assert!(job.contains("wayland-protocols"));

    assert!(job.contains("cargo build --release --locked ${{ matrix.cargo_features }}"));
    assert!(job.contains("cp packaging/linux/dev.wwwhynot3.slippy.desktop"));
    assert!(job.contains("cp packaging/linux/icons/hicolor/256x256/apps/dev.wwwhynot3.slippy.png"));
    assert!(!job.contains("cat > AppDir/usr/share/applications/slippy.desktop <<'DESKTOP'"));
    assert!(job.contains(
        "slippy-v${RELEASE_VERSION}-linux-${{ matrix.asset_arch }}-${{ matrix.backend }}.appimage"
    ));
    assert!(
        !job.contains("slippy-v${RELEASE_VERSION}-linux-${{ matrix.asset_arch }}-x11.AppImage")
    );
}

#[test]
fn linux_binary_release_matrix_keeps_raw_binary_and_uploads_bundle_tarballs() {
    let job = job_section(RELEASE_WORKFLOW, "  linux-binaries:", "  linux-appimages:");

    assert!(job.contains("name: Linux ${{ matrix.asset_arch }} ${{ matrix.backend }} binary"));
    assert!(job.contains(
        "dist/slippy-v${RELEASE_VERSION}-linux-${{ matrix.asset_arch }}-${{ matrix.backend }}"
    ));
    assert!(job.contains(
        "slippy-v${RELEASE_VERSION}-linux-${{ matrix.asset_arch }}-${{ matrix.backend }}-bundle.tar.gz"
    ));
    assert!(job.contains("tar -czf"));
    assert!(job.contains("packaging/linux/install-linux.sh"));
    assert!(job.contains("packaging/linux/uninstall-linux.sh"));
    assert!(job.contains("packaging/linux/dev.wwwhynot3.slippy.desktop"));
}

#[test]
fn windows_release_embeds_icon_before_uploading_executable() {
    let job = job_section(
        RELEASE_WORKFLOW,
        "  windows-binaries:",
        "  macos-universal-dmg:",
    );

    assert!(job.contains("name: Windows ${{ matrix.asset_arch }} binary"));
    assert!(job.contains("cargo build --release --locked --target ${{ matrix.target }}"));
    assert!(CARGO_TOML.contains("winresource"));
    assert!(BUILD_RS.contains("assets/icons/slippy.ico"));
    assert!(BUILD_RS.contains("WindowsResource"));
}

#[test]
fn macos_release_uploads_dmg_app_bundle_instead_of_bare_binary() {
    let macos_start = RELEASE_WORKFLOW
        .find("  macos-")
        .expect("missing macOS workflow job");
    let job = &RELEASE_WORKFLOW[macos_start..];

    assert!(job.contains("name: macOS universal DMG"));
    assert!(job.contains("app=\"dist/Slippy.app\""));
    assert!(job.contains("$app/Contents/MacOS"));
    assert!(job.contains("$app/Contents/Resources"));
    assert!(job.contains("Info.plist"));
    assert!(job.contains("iconutil -c icns"));
    assert!(job.contains("hdiutil create"));
    assert!(job.contains("slippy-v${RELEASE_VERSION}-macos-universal.dmg"));
    assert!(!job.contains("slippy-v${RELEASE_VERSION}-macos-universal\" --clobber"));
}
