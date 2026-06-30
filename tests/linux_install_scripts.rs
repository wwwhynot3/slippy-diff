const INSTALL_LINUX: &str = include_str!("../packaging/linux/install-linux.sh");
const UNINSTALL_LINUX: &str = include_str!("../packaging/linux/uninstall-linux.sh");
const INSTALL_RELEASE: &str = include_str!("../packaging/linux/install-release.sh");
const README: &str = include_str!("../README.md");

#[test]
fn linux_bundle_installer_installs_stable_uninstaller_command() {
    assert!(INSTALL_LINUX.contains("installed_uninstaller"));
    assert!(INSTALL_LINUX.contains(
        "install -m 755 \"${bundle_dir}/uninstall-linux.sh\" \"${installed_uninstaller}\""
    ));
    assert!(INSTALL_LINUX.contains("Run ${installed_uninstaller}"));
    assert!(UNINSTALL_LINUX.contains("rm -f \"${HOME}/.local/bin/slippy-uninstall\""));
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
