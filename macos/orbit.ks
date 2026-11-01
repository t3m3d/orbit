#!/usr/bin/env kr
// Orbit's macOS interface: KryptScript + Objective-K, no Swift/Obj-C UI.
import "k:cocoa"
import "k:objc"
import "head:cocoa"
import "head:objc"

func purpleWindow(win) {
    cocoaSetWindowBg(win, cocoaRGB(22, 16, 31))
    cocoaTransparentTitlebar(win)
    msg_1(win, "setAppearance:", msg_1(cls("NSAppearance"), "appearanceNamed:", nsString("NSAppearanceNameDarkAqua")))
    let content = msg(win, "contentView")
    msg_1(content, "setWantsLayer:", 1)
    msg_1(msg(content, "layer"), "setBackgroundColor:", msg(cocoaRGB(22, 16, 31), "CGColor"))
    emit 1
}
func purpleField(control) {
    cocoaSetDrawsBackground(control, 1)
    cocoaSetBg(control, cocoaRGB(36, 25, 52))
    cocoaSetTextColor(control, cocoaRGB(242, 234, 255))
    msg_1(control, "setFocusRingType:", 1)
    emit control
}
func inputH(win, x, y, w, h) {
    emit purpleField(cocoaTextField(win, x, y, w, h))
}
func alertH() {
    let alert = allocInit("NSAlert")
    purpleWindow(msg(alert, "window"))
    emit alert
}
func choosePath(title, folders) {
    let panel = msg(cls("NSOpenPanel"), "openPanel")
    msg_1(panel, "setTitle:", nsString(title))
    msg_1(panel, "setCanChooseFiles:", 1 - folders)
    msg_1(panel, "setCanChooseDirectories:", folders)
    msg_1(panel, "setAllowsMultipleSelection:", 0)
    purpleWindow(panel)
    if msg(panel, "runModal") != 1 { emit "" }
    emit str(msg(msg(panel, "URL"), "path"))
}

func appH() { emit cocoaSharedApp() }
func get(key) { emit cocoaGetAssocKey(appH(), "orbit." + key) }
func put(key, value) { cocoaSetAssocKey(appH(), "orbit." + key, value) emit 1 }
func dict() { emit msg(cls("NSMutableDictionary"), "dictionary") }
func set(d, key, value) { msg_2(d, "setObject:forKey:", value, nsString(key)) emit 1 }
func at(d, key) {
    let value = msg_1(d, "objectForKey:", nsString(key))
    if value == msg(cls("NSNull"), "null") { emit 0 }
    emit value
}
func str(o) { if o == 0 { emit "" } emit msg(msg(o, "description"), "UTF8String") }
func field(key) { emit cocoaGetText(get(key)) }
func jsonData(o) { emit msg_3(cls("NSJSONSerialization"), "dataWithJSONObject:options:error:", o, 0, 0) }
func decode(data) { if data == 0 { emit 0 } emit msg_3(cls("NSJSONSerialization"), "JSONObjectWithData:options:error:", data, 1, 0) }
func action(name) { let d = dict() set(d, "action", nsString(name)) emit d }
func send(d) {
    if msg(get("task"), "isRunning") != 1 { cocoaSetText(get("status"), "Torrent core stopped. Quit and reopen Orbit.") emit 0 }
    let data = jsonData(d)
    msg_1(get("input"), "writeData:", data)
    msg_1(get("input"), "writeData:", msg_1(nsString("\n"), "dataUsingEncoding:", 4))
    emit 1
}
func command(name) { send(action(name)) emit 1 }

// ── Browser handoff ────────────────────────────────────────────────────
// macOS records the *path* of the app that claims a scheme/content type.
// A build-folder copy is therefore a trap: rebuilding replaces the bundle and
// the association silently rots. Detect that, and report what macOS actually
// resolved instead of assuming the request worked.
func bundlePath() { emit str(msg(msg(cls("NSBundle"), "mainBundle"), "bundlePath")) }
func isInstalledPath() {
    let path = bundlePath()
    if indexOf(path, "/native/target/") >= 0 { emit 0 }
    if indexOf(path, "/var/folders/") >= 0 { emit 0 }
    if indexOf(path, "/private/tmp/") >= 0 { emit 0 }
    if indexOf(path, "/tmp/") >= 0 { emit 0 }
    emit 1
}
func samePath(a, b) {
    if a == "" || b == "" { emit 0 }
    let left = str(msg(nsString(a), "stringByResolvingSymlinksInPath"))
    let right = str(msg(nsString(b), "stringByResolvingSymlinksInPath"))
    if left == right { emit 1 }
    emit 0
}
func appPathForURL(url) {
    if url == 0 { emit "" }
    let application = msg_1(msg(cls("NSWorkspace"), "sharedWorkspace"), "URLForApplicationToOpenURL:", url)
    if application == 0 { emit "" }
    emit str(msg(application, "path"))
}
func magnetSample() { emit "magnet:?xt=urn:btih:0000000000000000000000000000000000000000" }
func torrentProbe() {
    let path = str(get("directory")) + "/link-handler.torrent"
    let manager = msg(cls("NSFileManager"), "defaultManager")
    // An empty descriptor only identifies the .torrent file type to LaunchServices.
    if msg_1(manager, "fileExistsAtPath:", nsString(path)) != 1 {
        msg_2(msg_1(nsString(""), "dataUsingEncoding:", 4), "writeToFile:atomically:", nsString(path), 1)
    }
    emit path
}
func currentMagnetHandler() { emit appPathForURL(cocoaURL(magnetSample())) }
func currentTorrentHandler() { emit appPathForURL(cocoaFileURL(torrentProbe())) }
func appName(path) {
    if path == "" { emit "no handler" }
    let name = msg_1(msg(cls("NSFileManager"), "defaultManager"), "displayNameAtPath:", nsString(path))
    if name == 0 { emit path }
    emit str(name)
}
func handlerPart(label, path) {
    if path == "" { emit label + ": no default app" }
    if samePath(path, bundlePath()) == 1 { emit label + ": this copy of Orbit" }
    emit label + ": " + appName(path) + " at " + path
}
func handlerReport() {
    emit handlerPart("Magnet links", currentMagnetHandler()) + "\n" + handlerPart(".torrent files", currentTorrentHandler())
}
func handlesBoth() {
    let ours = bundlePath()
    if samePath(currentMagnetHandler(), ours) == 0 { emit 0 }
    if samePath(currentTorrentHandler(), ours) == 0 { emit 0 }
    emit 1
}
func refreshHandler(announce) {
    cocoaSetToolTip(get("browser"), handlerReport())
    if handlesBoth() == 1 { cocoaSetTitle(get("browser"), "Browser Links: Orbit") }
    else { cocoaSetTitle(get("browser"), "Use Orbit for Browser Links…") }
    if announce == 1 && handlesBoth() == 1 {
        cocoaAlert("Browser links", "Magnet links and .torrent files now open in this copy of Orbit.\n\n" + handlerReport())
    }
    emit 1
}
// The layout is a fixed 1120x800 design; pinning the header to the top and
// letting the transfers table take the extra height keeps resizing usable.
func layout() {
    let content = msg(get("window"), "contentView")
    let width = msgRectWidth(content, "frame")
    let height = msgRectHeight(content, "frame")
    if width == cocoaNumberVal(get("lastwidth")) && height == cocoaNumberVal(get("lastheight")) { emit 0 }
    put("lastwidth", cocoaNumber(width))
    put("lastheight", cocoaNumber(height))
    cocoaSetFrame(get("headericon"), 24, height - 52, 30, 30)
    cocoaSetFrame(get("titlelabel"), 62, height - 54, 120, 34)
    cocoaSetFrame(get("subtitle"), 190, height - 49, 410, 24)
    cocoaSetFrame(get("status"), 620, height - 49, width - 644, 24)
    cocoaSetFrame(get("addlink"), 24, height - 134, 160, 30)
    cocoaSetFrame(get("opentorrent"), 196, height - 134, 170, 30)
    cocoaSetFrame(get("browser"), 378, height - 134, 260, 30)
    cocoaSetFrame(get("hint"), 660, height - 129, width - 684, 24)
    cocoaSetFrame(get("tablescroll"), 24, 384, width - 48, height - 536)
    cocoaSetFrame(get("previewtitle"), 24, 310, width - 50, 24)
    cocoaSetFrame(get("filesscroll"), 24, 192, width - 400, 110)
    cocoaSetFrame(get("destination"), 24, 150, width - 400, 28)
    cocoaSetFrame(get("add"), width - 200, 148, 170, 30)
    cocoaSetFrame(get("modesummary"), 180, 106, width - 204, 22)
    cocoaSetFrame(get("footnote1"), 24, 62, width - 48, 24)
    cocoaSetFrame(get("footnote2"), 24, 28, width - 48, 24)
    emit 1
}
func labelH(win, title, x, y, w, h) {
    let v = cocoaPlainLabel(win, title, x, y, w, h)
    cocoaSetTextColor(v, cocoaRGB(242, 234, 255))
    emit v
}
// The app icon as a header badge: same artwork as the Dock icon.
func headerIcon(win, x, y, size) {
    let view = msg_frame(msg(cls("NSImageView"), "alloc"), "initWithFrame:", x, y, size, size)
    let path = msg_2(msg(cls("NSBundle"), "mainBundle"), "pathForResource:ofType:", nsString("orbit-icon"), nsString("png"))
    if path == 0 { kp("header icon: orbit-icon.png missing from Resources") }
    if path != 0 {
        let img = msg_1(msg(cls("NSImage"), "alloc"), "initWithContentsOfFile:", path)
        if img == 0 { kp("header icon: orbit-icon.png could not be decoded") }
        if img != 0 { msg_1(view, "setImage:", img) }
    }
    msg_d1(view, "setImageScaling:", 3)
    msg_1(msg(win, "contentView"), "addSubview:", view)
    emit view
}
func buttonH(win, title, x, y, w) {
    let b = cocoaButton(win, title, x, y, w, 30)
    msg_1(b, "setBezelStyle:", 1)
    emit b
}
func rows() { emit at(get("state"), "torrents") }
func preview() { emit at(get("state"), "preview") }
func selected() {
    let index = msg(get("table"), "selectedRow")
    if index < 0 || index >= cocoaArrayCount(rows()) { emit 0 }
    emit cocoaArrayGet(rows(), index)
}
func rowCommand(name) {
    let row = selected()
    if row == 0 { cocoaSetText(get("status"), "Select a transfer first.") emit 0 }
    let d = action(name) set(d, "id", at(row, "id")) send(d) emit 1
}
func pauseSelected(self, cmd, sender) { rowCommand("pause") emit 1 }
func resumeSelected(self, cmd, sender) { rowCommand("resume") emit 1 }
func pauseAll(self, cmd, sender) { command("pause_all") emit 1 }
func resumeAll(self, cmd, sender) { command("resume_all") emit 1 }
func openFolder(self, cmd, sender) {
    let row = selected()
    if row != 0 { cocoaOpenPath(str(at(row, "output_folder"))) }
    else { cocoaOpenPath(field("destination")) }
    emit 1
}
func revealRow(self, cmd, sender) {
    let row = selected()
    if row != 0 { cocoaRevealPath(str(at(row, "output_folder"))) }
    emit 1
}
func copyMagnet(self, cmd, sender) {
    let row = selected()
    if row != 0 { cocoaSetClipboardText("magnet:?xt=urn:btih:" + str(at(row, "info_hash"))) }
    emit 1
}
func removeSelected(self, cmd, sender) {
    if selected() == 0 { emit 1 }
    let alert = alertH()
    msg_1(alert, "setMessageText:", nsString("Remove this transfer?"))
    msg_1(alert, "setInformativeText:", nsString("Keep Files removes it from Orbit. Delete Data permanently deletes its downloaded content."))
    msg_1(alert, "addButtonWithTitle:", nsString("Cancel"))
    msg_1(alert, "addButtonWithTitle:", nsString("Keep Files"))
    msg_1(alert, "addButtonWithTitle:", nsString("Delete Data"))
    let choice = msg(alert, "runModal")
    if choice == 1001 { rowCommand("forget") }
    if choice == 1002 { rowCommand("delete_data") }
    emit 1
}
func requestPreview(source) {
    if len(trim(source)) == 0 { emit 0 }
    cocoaArrayAdd(get("pending"), nsString(trim(source)))
    cocoaShow(get("window"), appH())
    emit 1
}
func onPreview(self, cmd, sender) {
    let alert = alertH()
    msg_1(alert, "setMessageText:", nsString("Add Link"))
    msg_1(alert, "setInformativeText:", nsString("Paste a magnet link or HTTP(S) torrent URL. Orbit previews files before starting a transfer."))
    msg_1(alert, "addButtonWithTitle:", nsString("Preview"))
    msg_1(alert, "addButtonWithTitle:", nsString("Cancel"))
    msg_1(alert, "setAccessoryView:", get("source"))
    msg_1(msg(alert, "window"), "setInitialFirstResponder:", get("source"))
    if msg(alert, "runModal") == 1000 { requestPreview(field("source")) }
    emit 1
}
func useBrowserLinks(self, cmd, sender) {
    let installed = isInstalledPath()
    let path = bundlePath()
    let alert = alertH()
    msg_1(alert, "setMessageText:", nsString("Use Orbit for browser links?"))
    let detail = "Make this copy of Orbit the macOS handler for magnet links and .torrent files. Your browser or macOS may ask before opening Orbit. HTTP torrent links download a .torrent file first; open that file with Orbit.\n\nCurrent handlers\n" + handlerReport()
    if installed == 0 {
        detail = "Orbit is running from a build folder:\n" + path + "\n\nmacOS records this exact location as the link handler, so browser links stop working when the build is replaced. Move Orbit.app to /Applications (or another permanent folder), open it there, and choose this again.\n\nCurrent handlers\n" + handlerReport()
    }
    msg_1(alert, "setInformativeText:", nsString(detail))
    msg_1(alert, "addButtonWithTitle:", nsString("Cancel"))
    if installed == 1 {
        msg_1(alert, "addButtonWithTitle:", nsString("Use Orbit"))
        if msg(alert, "runModal") != 1001 { emit 1 }
    } else {
        msg_1(alert, "addButtonWithTitle:", nsString("Reveal Orbit.app"))
        msg_1(alert, "addButtonWithTitle:", nsString("Use This Build Anyway"))
        let choice = msg(alert, "runModal")
        if choice == 1001 { cocoaRevealPath(path) emit 1 }
        if choice != 1002 { emit 1 }
    }
    let workspace = msg(cls("NSWorkspace"), "sharedWorkspace")
    let applicationURL = msg(msg(cls("NSBundle"), "mainBundle"), "bundleURL")
    msg_3(workspace, "setDefaultApplicationAtURL:toOpenURLsWithScheme:completionHandler:", applicationURL, nsString("magnet"), 0)
    msg_3(workspace, "setDefaultApplicationAtURL:toOpenContentTypeOfFileAtURL:completionHandler:", applicationURL, cocoaFileURL(torrentProbe()), 0)
    put("handlerchecks", cocoaNumber(0))
    put("handlerwait", cocoaNumber(1))
    cocoaSetToolTip(get("browser"), "Browser handoff requested. Waiting for macOS to confirm…")
    emit 1
}
func chooseTorrent(self, cmd, sender) {
    let path = choosePath("Open a .torrent file", 0)
    if path != "" { requestPreview(path) }
    emit 1
}
func chooseDestination(self, cmd, sender) {
    let path = choosePath("Orbit download folder", 1)
    if path != "" { cocoaSetText(get("destination"), path) }
    emit 1
}
func applySettings(self, cmd, sender) {
    let index = msg(get("mode"), "indexOfSelectedItem")
    let mode = "offline"
    if index == 1 { mode = "download_only" }
    if index == 2 { mode = "standard" }
    if index == 2 {
        let alert = alertH()
        msg_1(alert, "setMessageText:", nsString("Enable BitTorrent sharing?"))
        msg_1(alert, "setInformativeText:", nsString("Standard mode allows uploads. Trackers, peers, DHT and engine-managed PEX can reveal your IP address. This does not provide anonymity."))
        msg_1(alert, "addButtonWithTitle:", nsString("Cancel"))
        msg_1(alert, "addButtonWithTitle:", nsString("Enable Standard"))
        if msg(alert, "runModal") != 1001 { emit 1 }
    }
    let d = action("configure")
    set(d, "mode", nsString(mode))
    set(d, "destination", nsString(field("destination")))
    set(d, "dht", msg_1(cls("NSNumber"), "numberWithBool:", cocoaState(get("dht"))))
    set(d, "remember", msg_1(cls("NSNumber"), "numberWithBool:", cocoaState(get("remember"))))
    set(d, "down_kib", cocoaNumber(msg(get("down"), "intValue")))
    set(d, "up_kib", cocoaNumber(msg(get("up"), "intValue")))
    send(d)
    emit 1
}
// Networking lives in its own window so the main window stays a transfer list.
// It uses the same purple theme as the rest of the app, title bar included.
// AppKit still writes window-restore state for the bundle and then fails reading
// it on the next launch. Orbit restores nothing, so that state is dropped first.
func preparePersistence() {
    // Rely on the delegate refusing to save/restore state, not on AppKit's
    // ignore flag (which redirects state writes to a temp bundle and then
    // fails to restore them next launch).
    let defaults = msg(cls("NSUserDefaults"), "standardUserDefaults")
    msg_2(defaults, "setBool:forKey:", 0, nsString("NSQuitAlwaysKeepsWindows"))
    emit 1
}
// Shown once per Mac: the two notes that used to sit at the bottom of the
// window, so they are read before the first download.
func showWelcome() {
    let win = get("welcomewin")
    if win == 0 {
        win = cocoaWindow(appH(), "Orbit — Before You Start", 520, 330)
        msg_1(win, "setRestorable:", 0)
        purpleWindow(win)
        msg(win, "center")
        let heading = labelH(win, "ORBIT", 24, 280, 200, 30)
        cocoaSetTextColor(heading, cocoaRGB(183, 255, 60))
        labelH(win, "Downloads use Download Only by default, and uploads stay blocked until you enable Standard in Settings.", 24, 232, 470, 40)
        labelH(win, "DHT and PEX can expose your IP address.", 24, 206, 470, 24)
        labelH(win, "Closing this window quits Orbit and stops transfers.", 24, 164, 470, 24)
        labelH(win, "Settings and resume data stay on this Mac.", 24, 138, 470, 24)
        labelH(win, "Network mode, DHT and speed limits live in Settings (⌘,).", 24, 96, 470, 24)
        let ok = buttonH(win, "Got It", 386, 40, 110)
        cocoaOnClickKeyed(ok, "welcomeok", funcptr(closeWelcome))
        put("welcomewin", win)
    }
    cocoaShow(win, appH())
    emit 1
}
func closeWelcome(self, cmd, sender) { msg_1(get("welcomewin"), "orderOut:", 0) emit 1 }
func settingsControls(win) {
    labelH(win, "NETWORK", 24, 292, 300, 28)
    labelH(win, "Uploads stay blocked unless you choose Standard, which needs a confirmation.", 24, 262, 470, 22)
    let modes = cocoaArray()
    cocoaArrayAdd(modes, nsString("Offline"))
    cocoaArrayAdd(modes, nsString("Download Only (recommended)"))
    cocoaArrayAdd(modes, nsString("Standard (sharing)"))
    put("mode", popup(win, 24, 220, 300, modes))
    put("dht", cocoaCheckbox(win, "Use DHT for peer discovery", 24, 184, 320, 26))
    put("remember", cocoaCheckbox(win, "Remember this choice at startup", 24, 154, 320, 26))
    labelH(win, "Down / Up KiB/s (0 = unlimited)", 24, 116, 300, 22)
    put("down", inputH(win, 260, 112, 80, 26))
    put("up", inputH(win, 352, 112, 80, 26))
    let applyButton = buttonH(win, "Apply", 268, 58, 110)
    cocoaOnClickKeyed(applyButton, "settingsapply", funcptr(applySettings))
    put("settingsapply", applyButton)
    let closeButton = buttonH(win, "Close", 386, 58, 110)
    cocoaOnClickKeyed(closeButton, "settingsclose", funcptr(closeSettings))
    put("settingsclose", closeButton)
    emit 1
}
func openSettings(self, cmd, sender) {
    let win = get("settingswin")
    if win == 0 {
        win = cocoaWindow(appH(), "Orbit Settings", 520, 340)
        msg_1(win, "setRestorable:", 0)
        purpleWindow(win)
        msg(win, "center")
        settingsControls(win)
        put("settingswin", win)
    }
    if get("state") != 0 { syncSettingsControls(at(get("state"), "settings")) }
    cocoaShow(win, appH())
    emit 1
}
func closeSettings(self, cmd, sender) {
    msg_1(get("settingswin"), "orderOut:", 0)
    emit 1
}
func syncSettingsControls(settings) {
    let mode = str(at(settings, "network_mode"))
    let index = 0
    if mode == "download_only" { index = 1 }
    if mode == "standard" { index = 2 }
    msg_1(get("mode"), "selectItemAtIndex:", index)
    cocoaSetState(get("dht"), msg(at(settings, "dht_enabled"), "boolValue"))
    cocoaSetState(get("remember"), str(at(settings, "startup_network_policy")) == "remember_choice")
    cocoaSetText(get("down"), str(at(settings, "download_limit_kib")))
    cocoaSetText(get("up"), str(at(settings, "upload_limit_kib")))
    emit 1
}
func settingsSummary(state) {
    let settings = at(state, "settings")
    let mode = str(at(settings, "network_mode"))
    let label = "Offline"
    if mode == "download_only" { label = "Download Only" }
    if mode == "standard" { label = "Standard (sharing)" }
    let dht = "DHT off"
    if msg(at(settings, "dht_enabled"), "boolValue") == 1 { dht = "DHT on" }
    emit "Mode: " + label + "  ·  " + dht + "  ·  " + str(at(settings, "download_limit_kib")) + "/" + str(at(settings, "upload_limit_kib")) + " KiB/s"
}
// BEP 47 padding files only align real files to piece boundaries. They are not
// user content, so they stay selected for the transfer but stay out of the picker.
func paddingFile(name) {
    if indexOf(name, "___padding_file") >= 0 { emit 1 }
    emit 0
}
// The picker counts and indexes only real files, derived from the current
// preview on each query; nothing is cached between snapshots.
func visibleFileCount() {
    let files = at(preview(), "files")
    let total = cocoaArrayCount(files)
    let i = 0
    let n = 0
    while i < total {
        if paddingFile(str(at(cocoaArrayGet(files, i), "name"))) == 0 { n += 1 }
        i += 1
    }
    emit n
}
func visibleFileAt(index) {
    let files = at(preview(), "files")
    let total = cocoaArrayCount(files)
    let i = 0
    let seen = 0
    let found = 0
    while i < total {
        let file = cocoaArrayGet(files, i)
        if paddingFile(str(at(file, "name"))) == 0 {
            if seen == index { found = file }
            seen += 1
        }
        i += 1
    }
    emit found
}
// The transfer table and the file picker use separate delegate classes, so the
// picker's rows can never be painted as transfer rows.
func dsRows(self, cmd, table) {
    emit cocoaArrayCount(rows())
}
func fileRows(self, cmd, table) {
    emit visibleFileCount()
}
func fileValue(self, cmd, table, column, index) {
    let file = visibleFileAt(index)
    let text = ""
    if file != 0 {
        let marker = "[ ] "
        if msg(at(file, "selected"), "boolValue") == 1 { marker = "[✓] " }
        text = marker + str(at(file, "name")) + "   ·   " + str(at(file, "length")) + " bytes"
    }
    emit nsString(text)
}
func dsValue(self, cmd, table, column, index) {
    if index >= cocoaArrayCount(rows()) { emit nsString("") }
    let row = cocoaArrayGet(rows(), index)
    let key = str(msg(column, "identifier"))
    if key == "progress" {
        let total = msg(at(row, "total"), "longLongValue")
        let percent = 0
        if total > 0 { percent = msg(at(row, "downloaded"), "longLongValue") * 100 / total }
        emit nsString("" + percent + "%")
    }
    emit at(row, key)
}
func toggleFile(self, cmd, sender) {
    let file = visibleFileAt(msg(get("files"), "selectedRow"))
    if file == 0 { emit 1 }
    let enabled = 1 - msg(at(file, "selected"), "boolValue")
    set(file, "selected", msg_1(cls("NSNumber"), "numberWithBool:", enabled))
    cocoaReload(get("files"))
    emit 1
}
func addPreview(self, cmd, sender) {
    let draft = preview()
    if draft == 0 { emit 1 }
    let request = msg(draft, "mutableCopy")
    msg_1(request, "removeObjectForKey:", nsString("files"))
    msg_1(request, "removeObjectForKey:", nsString("total"))
    let files = at(draft, "files")
    let chosen = cocoaArray()
    let i = 0
    while i < cocoaArrayCount(files) {
        let file = cocoaArrayGet(files, i)
        if msg(at(file, "selected"), "boolValue") == 1 { cocoaArrayAdd(chosen, at(file, "index")) }
        i += 1
    }
    set(request, "selected_files", chosen)
    set(request, "output_folder", nsString(field("destination")))
    set(request, "start_paused", msg_1(cls("NSNumber"), "numberWithBool:", cocoaState(get("paused"))))
    let policy = "stop_when_complete"
    let choice = msg(get("policy"), "indexOfSelectedItem")
    if choice == 1 { policy = "seed" }
    if choice == 2 { policy = "remove_when_complete" }
    set(request, "share_policy", nsString(policy))
    let d = action("add") set(d, "request", request) send(d)
    msg(request, "release")
    emit 1
}
func cancelPreview(self, cmd, sender) { command("dismiss_preview") emit 1 }
func onQuit(self, cmd, sender) { put("quitting", cocoaNumber(1)) emit 1 }
func windowClose(self, cmd, sender) { onQuit(0, 0, 0) emit 0 }
func appTerminate(self, cmd, sender) { onQuit(0, 0, 0) emit 0 }
func appOpenFile(self, cmd, application, filename) { requestPreview(str(filename)) emit 1 }
func appOpenFiles(self, cmd, application, filenames) {
    let i = 0
    while i < cocoaArrayCount(filenames) { requestPreview(str(cocoaArrayGet(filenames, i))) i += 1 }
    msg_1(application, "replyToOpenOrPrint:", 0)
    emit 1
}
func appOpenURLs(self, cmd, application, urls) {
    let i = 0
    while i < cocoaArrayCount(urls) {
        let url = cocoaArrayGet(urls, i)
        if msg(url, "isFileURL") == 1 { requestPreview(str(msg(url, "path"))) }
        else { requestPreview(str(msg(url, "absoluteString"))) }
        i += 1
    }
    emit 1
}
func appReopen(self, cmd, application, visible) { cocoaShow(get("window"), appH()) emit 1 }
// Orbit has no window state worth restoring, and AppKit's restore prompt is what
// blocks launch behind "Reopen / Don't Reopen" after a crash.
func appNoRestore(self, cmd, application) { emit 0 }

func popup(win, x, y, w, values) {
    let control = msg_frame(msg(cls("NSPopUpButton"), "alloc"), "initWithFrame:", x, y, w, 28)
    msg_1(control, "setPullsDown:", 0)
    msg_1(msg(control, "cell"), "setArrowPosition:", 2)
    msg_1(control, "addItemsWithTitles:", values)
    _addToContentView(win, control)
    emit control
}
func columnH(table, key, title, width) {
    let column = msg_1(msg(cls("NSTableColumn"), "alloc"), "initWithIdentifier:", nsString(key))
    msg_1(column, "setTitle:", nsString(title))
    msg_d1(column, "setWidth:", width)
    msg_1(column, "setEditable:", 0)
    msg_1(table, "addTableColumn:", column)
    emit 1
}
func startCore() {
    let fm = msg(cls("NSFileManager"), "defaultManager")
    let temporary = msg(msg(fm, "temporaryDirectory"), "path")
    let unique = msg(msg(cls("NSUUID"), "UUID"), "UUIDString")
    let directory = str(temporary) + "/orbit-" + str(unique)
    let attrs = dict() set(attrs, "NSFilePosixPermissions", cocoaNumber(448))
    if msg_4(fm, "createDirectoryAtPath:withIntermediateDirectories:attributes:error:", nsString(directory), 1, attrs, 0) != 1 { emit 0 }
    put("directory", nsString(directory))
    put("statepath", nsString(directory + "/state.json"))
    let executable = environ("ORBIT_ENGINE_PATH")
    if executable == "" {
        let path = msg_1(msg(cls("NSBundle"), "mainBundle"), "pathForAuxiliaryExecutable:", nsString("orbit-engine"))
        executable = str(path)
    }
    let task = allocInit("NSTask")
    msg_1(task, "setExecutableURL:", cocoaFileURL(executable))
    let args = cocoaArray() cocoaArrayAdd(args, get("statepath"))
    msg_1(task, "setArguments:", args)
    let pipe = msg(cls("NSPipe"), "pipe")
    msg_1(task, "setStandardInput:", pipe)
    let log = directory + "/engine.log"
    msg_3(fm, "createFileAtPath:contents:attributes:", nsString(log), 0, attrs)
    let output = msg_1(cls("NSFileHandle"), "fileHandleForWritingAtPath:", nsString(log))
    msg_1(task, "setStandardOutput:", output)
    msg_1(task, "setStandardError:", output)
    if msg_1(task, "launchAndReturnError:", 0) == 0 { emit 0 }
    put("task", task)
    put("input", msg(pipe, "fileHandleForWriting"))
    emit 1
}
func pollCore() {
    let data = msg_1(cls("NSData"), "dataWithContentsOfFile:", get("statepath"))
    let state = decode(data)
    if state == 0 { emit 0 }
    let oldPreview = preview()
    let nextPreview = at(state, "preview")
    // Keep local file selections when the same preview arrives in a new snapshot.
    if oldPreview != 0 && nextPreview != 0 {
        if str(at(oldPreview, "info_hash")) == str(at(nextPreview, "info_hash")) { set(state, "preview", oldPreview) }
    }
    let selectedID = at(selected(), "id")
    put("state", state)
    let status = str(at(state, "status"))
    if at(state, "error") != 0 { status = str(at(state, "error")) }
    if cocoaArrayCount(get("pending")) > 0 {
        if str(at(at(state, "settings"), "network_mode")) == "offline" {
            status = "Link received — Orbit is in Offline mode. Open Settings to allow downloading."
        } else {
            if preview() == 0 && get("resolving") == 0 {
                let source = str(cocoaArrayGet(get("pending"), 0))
                let d = action("preview") set(d, "source", nsString(source))
                if send(d) == 1 { cocoaArrayRemove(get("pending"), 0) put("resolving", cocoaNumber(1)) }
            }
        }
    }
    if preview() != 0 || at(state, "error") != 0 { put("resolving", 0) }
    cocoaSetText(get("status"), status)
    cocoaSetToolTip(get("status"), status)
    cocoaSetText(get("modesummary"), settingsSummary(state))
    cocoaReload(get("table"))
    if selectedID != 0 {
        let i = 0
        while i < cocoaArrayCount(rows()) {
            if str(at(cocoaArrayGet(rows(), i), "id")) == str(selectedID) {
                let indexes = msg_1(cls("NSIndexSet"), "indexSetWithIndex:", i)
                msg_2(get("table"), "selectRowIndexes:byExtendingSelection:", indexes, 0)
            }
            i += 1
        }
    }
    cocoaReload(get("files"))
    if preview() == 0 { cocoaSetText(get("previewtitle"), "Preview a torrent to choose files before downloading.") }
    else { cocoaSetText(get("previewtitle"), str(at(preview(), "name"))) }
    cocoaSetEnabled(get("add"), preview() != 0)
    if get("loaded") == 0 {
        let settings = at(state, "settings")
        cocoaSetText(get("destination"), str(at(settings, "default_download_dir")))
        if get("settingswin") != 0 { syncSettingsControls(settings) }
        put("loaded", cocoaNumber(1))
        // A profile that was never configured starts in the normal mode: real
        // downloads with uploads blocked. Standard still needs the consent alert.
        if msg(at(settings, "network_setup_completed"), "boolValue") != 1 {
            let d = action("configure")
            set(d, "mode", nsString("download_only"))
            set(d, "destination", nsString(str(at(settings, "default_download_dir"))))
            set(d, "dht", msg_1(cls("NSNumber"), "numberWithBool:", 1))
            set(d, "remember", msg_1(cls("NSNumber"), "numberWithBool:", 1))
            set(d, "down_kib", cocoaNumber(0))
            set(d, "up_kib", cocoaNumber(0))
            send(d)
        }
        // Profiles configured by older builds inherited Offline as the default.
        // Move them to the normal mode once; Settings still allows Offline.
        if msg(at(settings, "network_setup_completed"), "boolValue") == 1 && str(at(settings, "network_mode")) == "offline" && get("modemigrated") == 0 {
            put("modemigrated", cocoaNumber(1))
            let m = action("configure")
            set(m, "mode", nsString("download_only"))
            set(m, "destination", nsString(str(at(settings, "default_download_dir"))))
            set(m, "dht", msg_1(cls("NSNumber"), "numberWithBool:", msg(at(settings, "dht_enabled"), "boolValue")))
            set(m, "remember", msg_1(cls("NSNumber"), "numberWithBool:", 1))
            set(m, "down_kib", cocoaNumber(msg(at(settings, "download_limit_kib"), "intValue")))
            set(m, "up_kib", cocoaNumber(msg(at(settings, "upload_limit_kib"), "intValue")))
            send(m)
        }
    }
    if get("loaded") != 0 && get("welcomed") == 0 {
        put("welcomed", cocoaNumber(1))
        let defaults = msg(cls("NSUserDefaults"), "standardUserDefaults")
        if msg_1(defaults, "boolForKey:", nsString("OrbitFirstRunSeen")) != 1 {
            msg_2(defaults, "setBool:forKey:", 1, nsString("OrbitFirstRunSeen"))
            if cocoaNumberVal(get("smoke")) != 1 { showWelcome() }
        }
    }
    if get("loaded") != 0 && get("handlerready") == 0 {
        refreshHandler(0)
        put("handlerready", cocoaNumber(1))
    }
    if get("handlerwait") != 0 {
        let checks = cocoaNumberVal(get("handlerchecks")) + 1
        put("handlerchecks", cocoaNumber(checks))
        if handlesBoth() == 1 {
            put("handlerwait", cocoaNumber(0))
            refreshHandler(1)
        } else {
            if checks >= 12 {
                put("handlerwait", cocoaNumber(0))
                refreshHandler(0)
                cocoaSetToolTip(get("browser"), handlerReport() + "\n\nmacOS did not confirm this copy as the handler. Move Orbit.app to /Applications, open it there, and try again.")
            }
        }
    }
    layout()
    emit 1
}

just run {
    let smoke = arg("0") == "--smoke"
    if smoke && environ("ORBIT_DATA_DIR") == "" { kp("FAIL: smoke requires isolated ORBIT_DATA_DIR") exit(1) }
    if smoke { put("smoke", cocoaNumber(1)) }
    // Orbit keeps no document or window state to restore. Without this, a past
    // crash makes macOS block launch behind its "reopen windows?" crash prompt.
    preparePersistence()
    let app = cocoaInit()
    let win = cocoaWindow(app, "Orbit · Krypton Torrent Manager", 1120, 800)
    put("window", win)
    msg_1(win, "setRestorable:", 0)
    cocoaSetWindowMinSize(win, 1080, 660)
    purpleWindow(win)
    msg(win, "center")
    let bar = cocoaMenuBar(app)
    let menu = cocoaMenuAdd(bar, "Orbit")
    cocoaMenuItem(menu, "Settings…", ",", funcptr(openSettings))
    cocoaMenuItem(menu, "Quit Orbit", "q", funcptr(onQuit))
    let file = cocoaMenuAdd(bar, "File")
    cocoaMenuItem(file, "Open Torrent…", "o", funcptr(chooseTorrent))
    cocoaMenuItem(file, "Add Link…", "l", funcptr(onPreview))
    cocoaMenuItem(file, "Use Orbit for Browser Links…", "", funcptr(useBrowserLinks))
    let edit = cocoaMenuAdd(bar, "Edit")
    cocoaMenuItemSel(edit, "Cut", "x", "cut:")
    cocoaMenuItemSel(edit, "Copy", "c", "copy:")
    cocoaMenuItemSel(edit, "Paste", "v", "paste:")
    cocoaMenuItemSel(edit, "Select All", "a", "selectAll:")
    let delegateClass = cocoaClassNew("OrbitDelegate")
    cocoaClassAddMethod(delegateClass, "windowShouldClose:", funcptr(windowClose), "c@:@")
    cocoaClassAddMethod(delegateClass, "applicationShouldTerminate:", funcptr(appTerminate), "q@:@")
    cocoaClassAddMethod(delegateClass, "application:openFile:", funcptr(appOpenFile), "c@:@@")
    cocoaClassAddMethod(delegateClass, "application:openFiles:", funcptr(appOpenFiles), "v@:@@")
    cocoaClassAddMethod(delegateClass, "application:openURLs:", funcptr(appOpenURLs), "v@:@@")
    cocoaClassAddMethod(delegateClass, "applicationShouldHandleReopen:hasVisibleWindows:", funcptr(appReopen), "c@:@c")
    cocoaClassAddMethod(delegateClass, "applicationShouldRestoreApplicationState:", funcptr(appNoRestore), "c@:@")
    cocoaClassAddMethod(delegateClass, "applicationShouldSaveApplicationState:", funcptr(appNoRestore), "c@:@")
    cocoaClassAddMethod(delegateClass, "numberOfRowsInTableView:", funcptr(dsRows), "q@:@")
    cocoaClassAddMethod(delegateClass, "tableView:objectValueForTableColumn:row:", funcptr(dsValue), "@@:@@q")
    cocoaClassAddMethod(delegateClass, "orbitReveal:", funcptr(revealRow), "v@:@")
    cocoaClassRegister(delegateClass)
    let delegate = cocoaNew(delegateClass)
    put("delegate", delegate)
    let filesClass = cocoaClassNew("OrbitFilesDelegate")
    cocoaClassAddMethod(filesClass, "numberOfRowsInTableView:", funcptr(fileRows), "q@:@")
    cocoaClassAddMethod(filesClass, "tableView:objectValueForTableColumn:row:", funcptr(fileValue), "@@:@@q")
    cocoaClassRegister(filesClass)
    let filesDelegate = cocoaNew(filesClass)
    msg_1(win, "setDelegate:", delegate)
    msg_1(app, "setDelegate:", delegate)
    put("headericon", headerIcon(win, 24, 748, 30))
    put("titlelabel", labelH(win, "ORBIT", 62, 746, 120, 34))
    put("subtitle", labelH(win, "Local transfers. Native Krypton.", 190, 751, 410, 24))
    put("status", labelH(win, "Starting…", 620, 751, 470, 24))
    cocoaSetTextColor(get("status"), cocoaRGB(183, 255, 60))
    // Paste field belongs to the Add Link dialog, never the main window.
    put("source", msg_frame(msg(cls("NSTextField"), "alloc"), "initWithFrame:", 0, 0, 470, 28))
    purpleField(get("source"))
    cocoaSetPlaceholder(get("source"), "magnet:?xt=… or https://…/download.torrent")
    put("pending", cocoaArray())
    let previewButton = buttonH(win, "Add Link…", 24, 666, 160)
    cocoaOnClickKeyed(previewButton, "preview", funcptr(onPreview))
    put("addlink", previewButton)
    let openButton = buttonH(win, "Open Torrent…", 196, 666, 170)
    cocoaOnClickKeyed(openButton, "open", funcptr(chooseTorrent))
    put("opentorrent", openButton)
    let browserButton = buttonH(win, "Use Orbit for Browser Links…", 378, 666, 260)
    cocoaOnClickKeyed(browserButton, "browser", funcptr(useBrowserLinks))
    put("browser", browserButton)
    put("hint", labelH(win, "Preview first. Choose files. Download.", 660, 671, 430, 24))
    put("table", cocoaTable(win, 24, 384, 1066, 264))
    put("tablescroll", msg(get("table"), "enclosingScrollView"))
    let table = get("table")
    msg_1(table, "removeTableColumn:", cocoaArrayGet(msg(table, "tableColumns"), 0))
    columnH(table, "name", "Transfer", 440)
    columnH(table, "status", "State", 130)
    columnH(table, "progress", "Progress", 100)
    columnH(table, "down_speed", "Download", 120)
    columnH(table, "up_speed", "Upload", 120)
    columnH(table, "eta", "ETA", 100)
    cocoaSetDataSource(table, delegate)
    msg_d1(table, "setRowHeight:", 30)
    msg_1(table, "setTarget:", delegate)
    msg_1(table, "setDoubleAction:", sel("orbitReveal:"))
    let pauseButton = buttonH(win, "Pause", 24, 344, 90)
    cocoaOnClickKeyed(pauseButton, "pause", funcptr(pauseSelected))
    let resumeButton = buttonH(win, "Resume", 122, 344, 90)
    cocoaOnClickKeyed(resumeButton, "resume", funcptr(resumeSelected))
    let pauseallButton = buttonH(win, "Pause All", 220, 344, 100)
    cocoaOnClickKeyed(pauseallButton, "pauseall", funcptr(pauseAll))
    let resumeallButton = buttonH(win, "Resume All", 328, 344, 100)
    cocoaOnClickKeyed(resumeallButton, "resumeall", funcptr(resumeAll))
    let folderButton = buttonH(win, "Show in Finder", 436, 344, 150)
    cocoaOnClickKeyed(folderButton, "folder", funcptr(openFolder))
    let magnetButton = buttonH(win, "Copy Magnet", 594, 344, 140)
    cocoaOnClickKeyed(magnetButton, "magnet", funcptr(copyMagnet))
    let removeButton = buttonH(win, "Remove…", 742, 344, 120)
    cocoaOnClickKeyed(removeButton, "remove", funcptr(removeSelected))
    put("previewtitle", labelH(win, "Preview a torrent to choose files before downloading.", 24, 310, 1060, 24))
    put("files", cocoaTable(win, 24, 192, 720, 110))
    put("filesscroll", msg(get("files"), "enclosingScrollView"))
    cocoaSetDataSource(get("files"), filesDelegate)
    msg_1(get("files"), "setHeaderView:", 0)
    let togglefileButton = buttonH(win, "Toggle Selected File", 764, 266, 326)
    cocoaOnClickKeyed(togglefileButton, "togglefile", funcptr(toggleFile))
    put("paused", cocoaCheckbox(win, "Start paused", 764, 233, 180, 26))
    let policies = cocoaArray()
    cocoaArrayAdd(policies, nsString("Stop when complete"))
    cocoaArrayAdd(policies, nsString("Keep seeding"))
    cocoaArrayAdd(policies, nsString("Remove when complete (keep files)"))
    put("policy", popup(win, 764, 198, 326, policies))
    put("destination", inputH(win, 24, 150, 720, 28))
    let destinationButton = buttonH(win, "Choose Folder…", 764, 148, 148)
    cocoaOnClickKeyed(destinationButton, "destination", funcptr(chooseDestination))
    put("add", buttonH(win, "Add Transfer", 920, 148, 170))
    cocoaOnClickKeyed(get("add"), "add", funcptr(addPreview))
    cocoaSetEnabled(get("add"), 0)
    let settingsButton = buttonH(win, "Settings…", 24, 101, 140)
    cocoaOnClickKeyed(settingsButton, "opensettings", funcptr(openSettings))
    put("settingsbutton", settingsButton)
    put("modesummary", labelH(win, "Mode: Offline", 180, 106, 700, 22))
    put("footnote1", labelH(win, "Downloads use Download Only by default and uploads stay blocked until you enable Standard in Settings. DHT / PEX expose your IP.", 24, 62, 1066, 24))
    put("footnote2", labelH(win, "Closing this window quits Orbit and stops transfers. Settings and resume data stay on this Mac.", 24, 28, 1066, 24))
    layout()
    if startCore() != 1 { cocoaAlert("Orbit could not start", "The bundled torrent engine could not launch.") exit(1) }
    cocoaShow(win, app)
    cocoaFinishLaunching(app)
    let frames = 0
    let smokeStage = 0
    while cocoaNumberVal(get("quitting")) != 1 {
        let pool = msg(cls("NSAutoreleasePool"), "new")
        cocoaPumpEvents(app)
        msg(app, "updateWindows")
        if frames % 20 == 0 { pollCore() }
        if msg(get("task"), "isRunning") != 1 {
            cocoaSetText(get("status"), "Torrent core stopped. Quit and reopen Orbit.")
            if smoke { kp("FAIL: core stopped") exit(1) }
        }
        if smoke && get("loaded") != 0 {
            if str(at(get("state"), "protocol")) != "1" { kp("FAIL: bridge protocol") exit(1) }
            if smokeStage == 0 {
                if msg(get("source"), "superview") != 0 { kp("FAIL: paste field visible in main window") exit(1) }
                msg_1(pauseButton, "performClick:", 0)
                if field("status") != "Select a transfer first." { kp("FAIL: native button callback") exit(1) }
                let urls = cocoaArray()
                cocoaArrayAdd(urls, cocoaURL("magnet:?xt=urn:btih:0000000000000000000000000000000000000000"))
                msg_2(delegate, "application:openURLs:", app, urls)
                if cocoaArrayCount(get("pending")) != 1 { kp("FAIL: magnet Apple event intake") exit(1) }
                cocoaArrayRemove(get("pending"), 0)
                if indexOf(bundlePath(), "Orbit.app") < 0 { kp("FAIL: bundle path detection") exit(1) }
                if len(handlerReport()) == 0 { kp("FAIL: browser handler report") exit(1) }
                if handlesBoth() == 1 && str(msg(get("browser"), "title")) != "Browser Links: Orbit" { kp("FAIL: handler state label") exit(1) }
                if handlesBoth() == 0 && str(msg(get("browser"), "title")) == "Browser Links: Orbit" { kp("FAIL: handler state label") exit(1) }
                let tableHeight = msgRectHeight(msg(get("tablescroll"), "frame"))
                msg_d2(win, "setContentSize:", 1200, 900)
                layout()
                if msgRectHeight(msg(get("tablescroll"), "frame")) <= tableHeight { kp("FAIL: resize layout") exit(1) }
                msg_d2(win, "setContentSize:", 1120, 800)
                layout()
                msg_1(settingsButton, "performClick:", 0)
                if get("settingswin") == 0 { kp("FAIL: settings window") exit(1) }
                msg_1(get("settingsapply"), "performClick:", 0)
                smokeStage = 1
            } else {
                if msg(at(at(get("state"), "settings"), "network_setup_completed"), "boolValue") == 1 {
                    kp("PASS: Objective-K callbacks, magnet intake, hidden paste field, settings bridge")
                    put("quitting", cocoaNumber(1))
                }
            }
        }
        if smoke && frames > 1000 { kp("FAIL: core startup timeout") exit(1) }
        msg(pool, "drain")
        sleepUs(0, 8000)
        frames += 1
    }
    command("shutdown")
    msg(get("input"), "closeFile")
    let waits = 0
    while msg(get("task"), "isRunning") == 1 && waits < 800 {
        sleepUs(0, 10000)
        waits += 1
    }
    if msg(get("task"), "isRunning") == 1 { msg(get("task"), "terminate") }
    msg_2(msg(cls("NSFileManager"), "defaultManager"), "removeItemAtPath:error:", get("directory"), 0)
    exit(0)
}
