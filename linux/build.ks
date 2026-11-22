#!/usr/bin/env kr
// Native Linux x86-64 builder, written in KryptScript.
import "k:sh"
import "k:env"
func quote(s) {
    let out = "'"
    let i = 0
    while i < len(s) {
        let c = substring(s, i, i + 1)
        if c == "'" { out += "'\"'\"'" } else { out += c }
        i += 1
    }
    emit out + "'"
}
func checked(command) {
    let output = exec("( " + command + " ) 2>&1; result=$?; printf '\n__ORBIT_STATUS_%s' \"$result\"")
    let marker = indexOf(output, "\n__ORBIT_STATUS_")
    if marker < 0 { print(output) exit(1) }
    let message = substring(output, 0, marker)
    if len(message) > 0 { print(message) }
    if substring(output, marker + 16, len(output)) != "0" { exit(1) }
    emit 1
}
just run {
    if sh("uname -sm") != "Linux x86_64" { print("Build on Linux x86-64, or use linux/Dockerfile.") exit(1) }
    let root = sh("pwd")
    checked("cargo build --locked --manifest-path " + quote(root + "/native/Cargo.toml") + " --release --bin orbit-native")
    let out = root + "/native/target/linux"
    checked("mkdir -p " + quote(out))
    let stage = sh("mktemp -d " + quote(root + "/native/target/linux-build.XXXXXX"))
    if stage == "" { print("Could not create build staging directory") exit(1) }
    let folder = stage + "/orbit-linux-x86_64"
    checked("mkdir -p " + quote(folder))
    checked("cp " + quote(root + "/native/target/release/orbit-native") + " " + quote(folder + "/orbit"))
    checked("cp " + quote(root + "/native/assets/orbit-icon-final.png") + " " + quote(folder + "/orbit.png"))
    checked("cp " + quote(root + "/LICENSE") + " " + quote(root + "/native/PRIVACY.md") + " " + quote(root + "/linux/README.md") + " " + quote(folder))
    checked("tar -C " + quote(stage) + " -czf " + quote(out + "/orbit-linux-x86_64.tar.gz") + " orbit-linux-x86_64")
    checked("cd " + quote(out) + " && sha256sum orbit-linux-x86_64.tar.gz > SHA256SUMS")
    checked("rm -rf " + quote(stage))
    print("Built " + out + "/orbit-linux-x86_64.tar.gz")
}
