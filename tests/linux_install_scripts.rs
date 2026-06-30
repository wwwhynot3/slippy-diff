const INSTALL_LINUX: &str = include_str!("../packaging/linux/install-linux.sh");
const UNINSTALL_LINUX: &str = include_str!("../packaging/linux/uninstall-linux.sh");
const INSTALL_RELEASE: &str = include_str!("../packaging/linux/install-release.sh");
const README: &str = include_str!("../README.md");
const UI_FLTK: &str = include_str!("../src/ui_fltk.rs");
const DESKTOP_ENTRY: &str = include_str!("../packaging/linux/dev.wwwhynot3.slippy.desktop");

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

#[test]
fn linux_bundle_installer_installs_stable_uninstaller_command() {
    assert!(INSTALL_LINUX.contains("installed_uninstaller"));
    assert!(INSTALL_LINUX.contains(
        "install -m 755 \"${bundle_dir}/uninstall-linux.sh\" \"${installed_uninstaller}\""
    ));
    assert!(INSTALL_LINUX.contains("sed -e \"s|^Exec=.*$|Exec=${installed_binary}|\""));
    assert!(INSTALL_LINUX.contains(
        "Some desktop environments may not show the launcher icon in search until you log out and back in."
    ));
    assert!(INSTALL_LINUX.contains("Run ${installed_uninstaller}"));
    assert!(UNINSTALL_LINUX.contains("rm -f \"${HOME}/.local/bin/slippy-uninstall\""));
}

#[test]
fn linux_desktop_entry_declares_startup_wm_class() {
    assert!(DESKTOP_ENTRY.contains("StartupWMClass=dev.wwwhynot3.slippy"));
}

#[test]
fn fltk_window_sets_matching_xclass_for_desktop_integration() {
    assert!(UI_FLTK.contains("const LINUX_DESKTOP_ID: &str = \"dev.wwwhynot3.slippy\";"));
    assert!(UI_FLTK.contains("window.set_xclass(LINUX_DESKTOP_ID);"));
}

#[test]
fn release_installer_downloads_latest_linux_bundle_by_default() {
    assert!(INSTALL_RELEASE.contains("github.com/${repo}/releases/latest"));
    assert!(INSTALL_RELEASE.contains("curl -fsSLI"));
    assert!(INSTALL_RELEASE.contains("[Ll]ocation:"));
    assert!(INSTALL_RELEASE.contains("api.github.com/repos/${repo}/releases/latest"));
    assert!(INSTALL_RELEASE.contains("Could not determine latest Slippy release tag."));
    assert!(INSTALL_RELEASE.contains("release_tag"));
    assert!(INSTALL_RELEASE.contains("slippy-${version}-linux-${arch}-${backend}-bundle.tar.gz"));
    assert!(INSTALL_RELEASE.contains("\"${bundle_root}/install-linux.sh\""));
    assert!(
        INSTALL_RELEASE
            .contains("https://github.com/${repo}/releases/download/${release_tag}/${asset}")
    );
    assert!(INSTALL_RELEASE.contains("version=\"${release_tag#slippy-}\""));
    assert!(INSTALL_RELEASE.contains("SLIPPY_ARCH"));
    assert!(INSTALL_RELEASE.contains("SLIPPY_BACKEND"));
    assert!(INSTALL_RELEASE.contains("uname -m"));
    assert!(INSTALL_RELEASE.contains("XDG_SESSION_TYPE"));
    assert!(INSTALL_RELEASE.contains("WAYLAND_DISPLAY"));
    assert!(INSTALL_RELEASE.contains("DISPLAY"));
    assert!(INSTALL_RELEASE.contains("echo \"amd64\""));
    assert!(INSTALL_RELEASE.contains("echo \"x11\""));
    assert!(INSTALL_RELEASE.contains("Resolved version: ${version}"));
    assert!(INSTALL_RELEASE.contains("Resolved release tag: ${release_tag}"));
    assert!(INSTALL_RELEASE.contains("Resolved arch: ${arch}"));
    assert!(INSTALL_RELEASE.contains("Resolved backend: ${backend}"));
    assert!(INSTALL_RELEASE.contains("Downloading asset: ${asset}"));
}

#[test]
fn readme_documents_one_command_linux_install() {
    assert!(README.contains("curl -fsSL https://raw.githubusercontent.com/wwwhynot3/slippy-diff/master/packaging/linux/install-release.sh | bash"));
    assert!(README.contains("~/.local/bin/slippy-uninstall"));
}

#[test]
fn install_linux_script_installs_desktop_file_and_icons_into_user_scope() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home_dir = temp.path().join("home");
    let bundle_dir = temp.path().join("bundle");
    let icon_256_src_dir = bundle_dir.join("share/icons/hicolor/256x256/apps");
    let icon_512_src_dir = bundle_dir.join("share/icons/hicolor/512x512/apps");
    let app_src_dir = bundle_dir.join("share/applications");

    fs::create_dir_all(&icon_256_src_dir).expect("icon 256 source dir");
    fs::create_dir_all(&icon_512_src_dir).expect("icon 512 source dir");
    fs::create_dir_all(&app_src_dir).expect("app source dir");
    fs::create_dir_all(home_dir.join(".local/bin")).expect("bin dir");

    let binary_path = bundle_dir.join("slippy");
    fs::write(&binary_path, "#!/usr/bin/env bash\nexit 0\n").expect("binary");
    fs::set_permissions(&binary_path, fs::Permissions::from_mode(0o755)).expect("binary perms");

    let uninstall_path = bundle_dir.join("uninstall-linux.sh");
    fs::write(&uninstall_path, "#!/usr/bin/env bash\nexit 0\n").expect("uninstaller");
    fs::set_permissions(&uninstall_path, fs::Permissions::from_mode(0o755))
        .expect("uninstaller perms");

    fs::write(
        app_src_dir.join("dev.wwwhynot3.slippy.desktop"),
        "[Desktop Entry]\nType=Application\nName=Slippy\nExec=slippy\nIcon=dev.wwwhynot3.slippy\nStartupWMClass=dev.wwwhynot3.slippy\n",
    )
    .expect("desktop");
    fs::copy(
        Path::new("packaging/linux/icons/hicolor/256x256/apps/dev.wwwhynot3.slippy.png"),
        icon_256_src_dir.join("dev.wwwhynot3.slippy.png"),
    )
    .expect("copy icon");
    fs::copy(
        Path::new("packaging/linux/icons/hicolor/512x512/apps/dev.wwwhynot3.slippy.png"),
        icon_512_src_dir.join("dev.wwwhynot3.slippy.png"),
    )
    .expect("copy 512 icon");

    let install_path = bundle_dir.join("install-linux.sh");
    fs::write(&install_path, INSTALL_LINUX).expect("install script");
    fs::set_permissions(&install_path, fs::Permissions::from_mode(0o755)).expect("install perms");

    let status = Command::new("bash")
        .arg(&install_path)
        .env("HOME", &home_dir)
        .status()
        .expect("run install");
    assert!(status.success());

    assert!(home_dir.join(".local/bin/slippy").exists());
    assert!(home_dir.join(".local/bin/slippy-uninstall").exists());
    assert!(
        home_dir
            .join(".local/share/applications/dev.wwwhynot3.slippy.desktop")
            .exists()
    );
    assert!(
        home_dir
            .join(".local/share/icons/hicolor/256x256/apps/dev.wwwhynot3.slippy.png")
            .exists()
    );
    assert!(
        home_dir
            .join(".local/share/icons/hicolor/512x512/apps/dev.wwwhynot3.slippy.png")
            .exists()
    );

    let desktop_entry = fs::read_to_string(
        home_dir.join(".local/share/applications/dev.wwwhynot3.slippy.desktop"),
    )
    .expect("read desktop entry");
    assert!(desktop_entry.contains(&format!(
        "Exec={}",
        home_dir.join(".local/bin/slippy").display()
    )));
    assert!(desktop_entry.contains("Icon=dev.wwwhynot3.slippy"));
    assert!(desktop_entry.contains("StartupWMClass=dev.wwwhynot3.slippy"));
}
