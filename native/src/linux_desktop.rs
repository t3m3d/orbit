use anyhow::{Context, Result, bail};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

const DESKTOP_ID: &str = "org.krypton-lang.orbit.desktop";
const TYPES: [&str; 2] = ["application/x-bittorrent", "x-scheme-handler/magnet"];

pub fn normalize_source(source: String) -> Option<String> {
    if source.to_ascii_lowercase().starts_with("file:") {
        let path = url::Url::parse(&source).ok()?.to_file_path().ok()?;
        return Some(path.to_string_lossy().into_owned());
    }
    Some(source)
}

// Desktop Exec syntax is not shell syntax. Quote a single literal path and
// escape percent field codes so names such as 100%/Orbit remain literal.
fn exec_path(path: &Path) -> Result<String> {
    let path = path.to_str().context("desktop path must be valid UTF-8")?;
    if path.contains(['\n', '\r']) {
        bail!("desktop path cannot contain newlines");
    }
    let mut out = String::from("\"");
    for c in path.chars() {
        match c {
            '%' => out.push_str("%%"),
            '\\' => out.push_str("\\\\\\\\"),
            '"' | '$' | '`' => {
                out.push_str("\\\\");
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    Ok(out)
}

fn desktop_entry(executable: &Path) -> Result<String> {
    Ok(format!(
        "[Desktop Entry]\nVersion=1.0\nType=Application\nName=Orbit Torrent Manager\nComment=Local-first BitTorrent transfers\nExec={} %U\nIcon=org.krypton-lang.orbit\nTerminal=false\nCategories=Network;FileTransfer;P2P;\nMimeType=application/x-bittorrent;x-scheme-handler/magnet;\nStartupWMClass=Orbit\n",
        exec_path(executable)?
    ))
}

pub fn install() -> Result<()> {
    let home = dirs::home_dir().context("home directory is unavailable")?;
    let data = dirs::data_local_dir().context("XDG data directory is unavailable")?;
    let directory = home.join(".local/lib/orbit");
    fs::create_dir_all(&directory)?;
    let binary = directory.join("orbit");
    let source = std::env::current_exe()?;
    if fs::canonicalize(&source)? != fs::canonicalize(&binary).unwrap_or_default() {
        let stage = directory.join(format!(".orbit-{}", std::process::id()));
        fs::copy(&source, &stage).context("could not install Orbit executable")?;
        fs::set_permissions(&stage, fs::Permissions::from_mode(0o755))?;
        fs::rename(stage, &binary)?;
    }
    let applications = data.join("applications");
    fs::create_dir_all(&applications)?;
    fs::write(applications.join(DESKTOP_ID), desktop_entry(&binary)?)?;
    let icons = data.join("icons/hicolor/256x256/apps");
    fs::create_dir_all(&icons)?;
    fs::write(
        icons.join("org.krypton-lang.orbit.png"),
        include_bytes!("../assets/orbit-icon-final.png"),
    )?;
    // Optional cache refresh. xdg-mime works with the desktop file directly.
    let _ = Command::new("update-desktop-database")
        .arg(&applications)
        .output();
    Ok(())
}

pub fn make_default() -> Result<()> {
    install()?;
    for mime in TYPES {
        let output = Command::new("xdg-mime")
            .args(["default", DESKTOP_ID, mime])
            .output()
            .context("xdg-mime is required; install xdg-utils")?;
        if !output.status.success() {
            bail!(
                "xdg-mime failed for {mime}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let output = Command::new("xdg-mime")
            .args(["query", "default", mime])
            .output()?;
        if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != DESKTOP_ID
        {
            bail!("the desktop did not confirm Orbit as the default for {mime}");
        }
    }
    Ok(())
}

pub fn handle_cli() -> Option<Result<()>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--install-desktop") {
        return Some(
            install().map(|()| println!("Installed Orbit desktop launcher for this user")),
        );
    }
    if args.iter().any(|arg| arg == "--make-default") {
        return Some(
            make_default().map(|()| println!("Orbit handles .torrent files and magnet links")),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_exec_quotes_literal_paths_and_preserves_url_field() {
        let entry = desktop_entry(Path::new("/home/Test User/100%/orbit")).unwrap();
        assert!(entry.contains("Exec=\"/home/Test User/100%%/orbit\" %U\n"));
        assert!(entry.contains("MimeType=application/x-bittorrent;x-scheme-handler/magnet;"));
        assert!(desktop_entry(Path::new("/bad\npath")).is_err());
    }
    #[test]
    fn desktop_exec_escapes_desktop_and_argument_layers() {
        assert_eq!(
            exec_path(Path::new("/a/\"$`\\b")).unwrap(),
            "\"/a/\\\\\"\\\\$\\\\`\\\\\\\\b\""
        );
    }

    #[test]
    fn desktop_file_urls_become_local_paths_without_losing_spaces() {
        assert_eq!(
            normalize_source("file:///tmp/Legal%20file.torrent".into()).as_deref(),
            Some("/tmp/Legal file.torrent")
        );
        assert!(normalize_source("file://remote-host/tmp/file.torrent".into()).is_none());
        assert_eq!(
            normalize_source("magnet:?xt=urn:btih:abc".into()).as_deref(),
            Some("magnet:?xt=urn:btih:abc")
        );
    }
}
