#!/usr/bin/env kr
// Run from the Orbit checkout: kcc -r macos/build.ks
import "k:sh"
import "k:env"

func quote(s) {
    let result = "'"
    let i = 0
    while i < len(s) {
        let c = substring(s, i, i + 1)
        if c == "'" { result += "'\"'\"'" } else { result += c }
        i += 1
    }
    emit result + "'"
}
func checked(command) {
    let output = exec("( " + command + " ) 2>&1; status=$?; printf '\n__ORBIT_STATUS_%s' \"$status\"")
    let marker = indexOf(output, "\n__ORBIT_STATUS_")
    if marker < 0 { kp(output) exit(1) }
    let message = substring(output, 0, marker)
    if len(message) > 0 { kp(message) }
    if substring(output, marker + 16, len(output)) != "0" { exit(1) }
    emit 1
}
just run {
    if sh("uname -sm") != "Darwin arm64" { kp("This build targets Apple Silicon macOS.") exit(1) }
    let root = sh("pwd")
    let krypton = env("KRYPTON_ROOT")
    if krypton == "" { krypton = root + "/../krypton" }
    let compiler = krypton + "/kcc-native"
    let profile = "release"
    let releaseFlag = " --release"
    if arg("0") == "--debug" { profile = "debug" releaseFlag = "" }
    checked("cargo build --locked --no-default-features --manifest-path " + quote(root + "/native/Cargo.toml") + releaseFlag + " --bin orbit-engine")
    checked("mkdir -p " + quote(root + "/native/target"))
    let stage = sh("mktemp -d " + quote(root + "/native/target/macos-build.XXXXXX"))
    if stage == "" { kp("Could not create staging directory") exit(1) }
    let app = stage + "/Orbit.app"
    checked("mkdir -p " + quote(app + "/Contents/MacOS") + " " + quote(app + "/Contents/Resources"))
    checked("env KRYPTON_ROOT=" + quote(krypton) + " " + quote(compiler) + " " + quote(root + "/macos/orbit.ks") + " -o " + quote(app + "/Contents/MacOS/Orbit"))
    checked("cp " + quote(root + "/native/target/" + profile + "/orbit-engine") + " " + quote(app + "/Contents/MacOS/orbit-engine"))
    checked("cp " + quote(root + "/macos/Info.plist") + " " + quote(app + "/Contents/Info.plist"))
    // Reuse the existing Orbit artwork for Finder and the Dock.
    // Reuse the Orbit artwork for Finder and the Dock. The source may be a PNG
    // or a JPG, so normalise it to a PNG once and resize every icon from that.
    let icons = stage + "/Orbit.iconset"
    checked("mkdir -p " + quote(icons))
    let art = stage + "/orbit-icon-source.png"
    checked("sips -s format png " + quote(root + "/native/assets/orbit-icon-final.png") + " --out " + quote(art) + " >/dev/null || sips -s format png " + quote(root + "/native/assets/orbit-icon-final.jpg") + " --out " + quote(art) + " >/dev/null")
    let size = 16
    while size <= 512 {
        checked("sips -s format png -z " + size + " " + size + " " + quote(art) + " --out " + quote(icons + "/icon_" + size + "x" + size + ".png") + " >/dev/null")
        let retina = size * 2
        checked("sips -s format png -z " + retina + " " + retina + " " + quote(art) + " --out " + quote(icons + "/icon_" + size + "x" + size + "@2x.png") + " >/dev/null")
        size *= 2
    }
    checked("iconutil -c icns " + quote(icons) + " -o " + quote(app + "/Contents/Resources/Orbit.icns"))
    checked("sips -s format png -Z 128 " + quote(root + "/native/assets/orbit-grayscale-badge.png") + " --out " + quote(app + "/Contents/Resources/orbit-icon.png") + " >/dev/null")
    checked("cp " + quote(root + "/LICENSE") + " " + quote(app + "/Contents/Resources/LICENSE"))
    checked("cp " + quote(root + "/native/PRIVACY.md") + " " + quote(app + "/Contents/Resources/PRIVACY.md"))
    checked("plutil -lint " + quote(app + "/Contents/Info.plist"))
    let destination = root + "/native/target/Orbit.app"
    checked("rm -rf " + quote(destination))
    checked("mv " + quote(app) + " " + quote(destination))
    checked("rm -rf " + quote(stage))
    kp("Built " + destination + " (" + profile + ")")
}
