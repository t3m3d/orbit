param(
    [Parameter(Mandatory = $false)]
    [string]$ExecutablePath = (Join-Path $PSScriptRoot 'Orbit.exe')
)

$resolvedExecutable = (Resolve-Path -LiteralPath $ExecutablePath -ErrorAction Stop).Path
$openCommand = '"' + $resolvedExecutable + '" "%1"'
$installedIconPath = Join-Path (Split-Path -Parent $resolvedExecutable) 'Orbit.ico'
$icon = if (Test-Path -LiteralPath $installedIconPath) {
    '"' + (Resolve-Path -LiteralPath $installedIconPath).Path + '"'
} else {
    '"' + $resolvedExecutable + '",0'
}

function Ensure-Key([string]$Path) {
    New-Item -Path $Path -Force | Out-Null
}

function Set-DefaultValue([string]$Path, [string]$Value) {
    Ensure-Key $Path
    Set-Item -Path $Path -Value $Value
}

function Set-StringValue([string]$Path, [string]$Name, [string]$Value) {
    Ensure-Key $Path
    New-ItemProperty -Path $Path -Name $Name -Value $Value -PropertyType String -Force | Out-Null
}

$torrentProgId = 'HKCU:\Software\Classes\Orbit.Torrent'
Set-DefaultValue $torrentProgId 'Orbit Torrent File'
Set-DefaultValue "$torrentProgId\DefaultIcon" $icon
Set-DefaultValue "$torrentProgId\shell\open\command" $openCommand

$magnetProgId = 'HKCU:\Software\Classes\Orbit.Magnet'
Set-DefaultValue $magnetProgId 'URL:Magnet Protocol'
Set-StringValue $magnetProgId 'URL Protocol' ''
Set-DefaultValue "$magnetProgId\DefaultIcon" $icon
Set-DefaultValue "$magnetProgId\shell\open\command" $openCommand

$torrentExtension = 'HKCU:\Software\Classes\.torrent'
Set-DefaultValue $torrentExtension 'Orbit.Torrent'
Set-StringValue "$torrentExtension\OpenWithProgids" 'Orbit.Torrent' ''

$magnetScheme = 'HKCU:\Software\Classes\magnet'
Set-DefaultValue $magnetScheme 'URL:Magnet Protocol'
Set-StringValue $magnetScheme 'URL Protocol' ''
Set-DefaultValue "$magnetScheme\DefaultIcon" $icon
Set-DefaultValue "$magnetScheme\shell\open\command" $openCommand

$application = 'HKCU:\Software\Classes\Applications\Orbit.exe'
Set-StringValue $application 'FriendlyAppName' 'Orbit'
Set-DefaultValue "$application\DefaultIcon" $icon
Set-DefaultValue "$application\shell\open\command" $openCommand
Set-StringValue "$application\SupportedTypes" '.torrent' ''

$capabilities = 'HKCU:\Software\Krypton\Orbit\Capabilities'
Set-StringValue $capabilities 'ApplicationName' 'Orbit'
Set-StringValue $capabilities 'ApplicationDescription' 'Krypton-themed native BitTorrent client for magnet links and .torrent files.'
Set-StringValue $capabilities 'ApplicationIcon' $icon
Set-StringValue "$capabilities\FileAssociations" '.torrent' 'Orbit.Torrent'
Set-StringValue "$capabilities\UrlAssociations" 'magnet' 'Orbit.Magnet'
Set-StringValue 'HKCU:\Software\RegisteredApplications' 'Orbit' 'Software\Krypton\Orbit\Capabilities'

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class OrbitShellRegistration {
    [DllImport("shell32.dll")]
    public static extern void SHChangeNotify(uint eventId, uint flags, IntPtr item1, IntPtr item2);
}
"@
[OrbitShellRegistration]::SHChangeNotify(0x08000000, 0x1000, [IntPtr]::Zero, [IntPtr]::Zero)

Write-Output "Orbit registered for .torrent files and magnet links."
Write-Output "Executable: $resolvedExecutable"

