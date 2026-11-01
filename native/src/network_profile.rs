use crate::settings::SystemNetworkProfile;

#[cfg(any(windows, test))]
fn parse_profile_probe(output: &str) -> SystemNetworkProfile {
    match output.trim().lines().last().unwrap_or_default().trim() {
        "disconnected" => SystemNetworkProfile::Disconnected,
        "metered" => SystemNetworkProfile::Metered,
        "public" => SystemNetworkProfile::Public,
        "private" => SystemNetworkProfile::Private,
        _ => SystemNetworkProfile::Unknown,
    }
}

#[cfg(windows)]
pub fn detect_system_network_profile() -> SystemNetworkProfile {
    use std::process::Command;

    const PROBE: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
try {
  $null = [Windows.Networking.Connectivity.NetworkInformation, Windows.Networking.Connectivity, ContentType=WindowsRuntime]
  $connection = [Windows.Networking.Connectivity.NetworkInformation]::GetInternetConnectionProfile()
  if ($null -eq $connection -or $connection.GetNetworkConnectivityLevel().ToString() -eq 'None') {
    'disconnected'
    exit
  }
  $cost = $connection.GetConnectionCost().NetworkCostType.ToString()
  if ($cost -eq 'Fixed' -or $cost -eq 'Variable') {
    'metered'
    exit
  }
} catch {}

$profiles = @(Get-NetConnectionProfile | Where-Object {
  $_.IPv4Connectivity -ne 'Disconnected' -or $_.IPv6Connectivity -ne 'Disconnected'
})
if ($profiles.Count -eq 0) {
  'unknown'
} elseif ($profiles.NetworkCategory -contains 'Public') {
  'public'
} else {
  'private'
}
"#;

    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", PROBE])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or(SystemNetworkProfile::Unknown, |output| {
            parse_profile_probe(&output)
        })
}

#[cfg(not(windows))]
pub fn detect_system_network_profile() -> SystemNetworkProfile {
    SystemNetworkProfile::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_probe_parser_defaults_safely() {
        assert_eq!(
            parse_profile_probe("private\r\n"),
            SystemNetworkProfile::Private
        );
        assert_eq!(
            parse_profile_probe("metered\n"),
            SystemNetworkProfile::Metered
        );
        assert_eq!(
            parse_profile_probe("access denied"),
            SystemNetworkProfile::Unknown
        );
    }
}
