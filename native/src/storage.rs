use std::path::Path;

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes == 0 {
        return "0 B".to_owned();
    }
    let unit = ((bytes as f64).log(1024.0).floor() as usize).min(UNITS.len() - 1);
    let value = bytes as f64 / 1024_f64.powi(unit as i32);
    if value >= 10.0 || unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(windows)]
fn available_disk_space(path: &Path) -> Result<Option<u64>, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let probe = path
        .ancestors()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| "The destination drive or parent folder does not exist.".to_owned())?;
    let mut wide = probe.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    let mut available = 0_u64;
    let success = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        Err(format!(
            "Could not read free space for {}: {}",
            probe.display(),
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(Some(available))
    }
}

#[cfg(not(any(windows, unix)))]
fn available_disk_space(_path: &Path) -> Result<Option<u64>, String> {
    Ok(None)
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)] // statvfs field widths differ across Unix targets.
fn available_disk_space(path: &Path) -> Result<Option<u64>, String> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let probe = path
        .ancestors()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| "The destination volume or parent folder does not exist.".to_owned())?;
    let path = CString::new(probe.as_os_str().as_bytes()).map_err(|error| error.to_string())?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // statvfs writes the structure only on success; the C string lives through the call.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(format!(
            "Could not read free space for {}: {}",
            probe.display(),
            std::io::Error::last_os_error()
        ));
    }
    let stats = unsafe { stats.assume_init() };
    Ok(Some(
        (stats.f_bavail as u64).saturating_mul(stats.f_frsize as u64),
    ))
}

pub fn storage_preflight(destination: &str, required: u64) -> Result<Option<u64>, String> {
    let destination = destination.trim();
    if destination.is_empty() {
        return Err("Choose a destination folder.".to_owned());
    }
    let path = Path::new(destination);
    if !path.is_absolute() {
        return Err("The destination must be an absolute path.".to_owned());
    }
    if path.exists() && !path.is_dir() {
        return Err("The destination points to a file, not a folder.".to_owned());
    }

    let available = available_disk_space(path)?;
    if available.is_some_and(|available| required > available) {
        Err(format!(
            "Not enough free space: {} required, {} available.",
            format_bytes(required),
            format_bytes(available.unwrap_or_default())
        ))
    } else {
        Ok(available)
    }
}
