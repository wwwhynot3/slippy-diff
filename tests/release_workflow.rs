const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release-please.yml");

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
    assert!(job.contains(
        "slippy-v${RELEASE_VERSION}-linux-${{ matrix.asset_arch }}-${{ matrix.backend }}.appimage"
    ));
    assert!(
        !job.contains("slippy-v${RELEASE_VERSION}-linux-${{ matrix.asset_arch }}-x11.AppImage")
    );
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
