#!/usr/bin/env kr
// Orbit's macOS interface: KryptScript + Objective-K, no Swift/Obj-C UI.
import "k:cocoa"
import "k:objc"
import "head:cocoa"
import "head:objc"

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
func labelH(win, title, x, y, w, h) {
    let v = cocoaPlainLabel(win, title, x, y, w, h)
    cocoaSetTextColor(v, cocoaRGB(242, 234, 255))
    emit v
}
func buttonH(win, title, x, y, w, key, callback) {
    let b = cocoaButton(win, title, x, y, w, 30)
    msg_1(b, "setBezelStyle:", 1)
    cocoaOnClickKeyed(b, key, callback)
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
func copyMagnet(self, cmd, sender) {
    let row = selected()
    if row != 0 { cocoaSetClipboardText("magnet:?xt=urn:btih:" + str(at(row, "info_hash"))) }
    emit 1
}
func removeSelected(self, cmd, sender) {
    if selected() == 0 { emit 1 }
    let alert = allocInit("NSAlert")
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
    let alert = allocInit("NSAlert")
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
    let alert = allocInit("NSAlert")
    msg_1(alert, "setMessageText:", nsString("Use Orbit for browser links?"))
    msg_1(alert, "setInformativeText:", nsString("Make this Orbit app the handler for magnet links and .torrent files. Your browser may ask before opening Orbit. HTTP torrent links download a .torrent file first; open that file with Orbit."))
    msg_1(alert, "addButtonWithTitle:", nsString("Cancel"))
    msg_1(alert, "addButtonWithTitle:", nsString("Use Orbit"))
    if msg(alert, "runModal") != 1001 { emit 1 }
    let workspace = msg(cls("NSWorkspace"), "sharedWorkspace")
    let applicationURL = msg(msg(cls("NSBundle"), "mainBundle"), "bundleURL")
    msg_3(workspace, "setDefaultApplicationAtURL:toOpenURLsWithScheme:completionHandler:", applicationURL, nsString("magnet"), 0)
    let descriptor = str(get("directory")) + "/association.torrent"
    // Use an empty local descriptor only to identify its file type to LaunchServices.
    let empty = msg_1(nsString(""), "dataUsingEncoding:", 4)
    msg_2(empty, "writeToFile:atomically:", nsString(descriptor), 1)
    msg_3(workspace, "setDefaultApplicationAtURL:toOpenContentTypeOfFileAtURL:completionHandler:", applicationURL, cocoaFileURL(descriptor), 0)
    cocoaSetText(get("status"), "Browser handoff requested. Accept any macOS or browser confirmation.")
    emit 1
}
func chooseTorrent(self, cmd, sender) {
    let path = cocoaChooseFile("Open a .torrent file")
    if path != "" { requestPreview(path) }
    emit 1
}
func chooseDestination(self, cmd, sender) {
    let path = cocoaChooseFolder("Orbit download folder")
    if path != "" { cocoaSetText(get("destination"), path) }
    emit 1
}
func applySettings(self, cmd, sender) {
    let index = msg(get("mode"), "indexOfSelectedItem")
    let mode = "offline"
    if index == 1 { mode = "download_only" }
    if index == 2 { mode = "standard" }
    if index == 2 {
        let alert = allocInit("NSAlert")
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
func dsRows(self, cmd, table) {
    if table == get("files") { emit cocoaArrayCount(at(preview(), "files")) }
    emit cocoaArrayCount(rows())
}
func dsValue(self, cmd, table, column, index) {
    if table == get("files") {
        let files = at(preview(), "files")
        if index >= cocoaArrayCount(files) { emit nsString("") }
        let file = cocoaArrayGet(files, index)
        let marker = "[ ] "
        if msg(at(file, "selected"), "boolValue") == 1 { marker = "[✓] " }
        emit nsString(marker + str(at(file, "name")) + "   ·   " + str(at(file, "length")) + " bytes")
    }
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
    let index = msg(get("files"), "selectedRow")
    let files = at(preview(), "files")
    if index < 0 || index >= cocoaArrayCount(files) { emit 1 }
    let file = cocoaArrayGet(files, index)
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
            status = "Link received — choose Download Only or Standard, then Apply Network Settings."
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
        let mode = str(at(settings, "network_mode"))
        let index = 0
        if mode == "download_only" { index = 1 }
        if mode == "standard" { index = 2 }
        msg_1(get("mode"), "selectItemAtIndex:", index)
        cocoaSetState(get("dht"), msg(at(settings, "dht_enabled"), "boolValue"))
        cocoaSetState(get("remember"), str(at(settings, "startup_network_policy")) == "remember_choice")
        cocoaSetText(get("down"), str(at(settings, "download_limit_kib")))
        cocoaSetText(get("up"), str(at(settings, "upload_limit_kib")))
        put("loaded", cocoaNumber(1))
    }
    emit 1
}

just run {
    let app = cocoaInit()
    let win = cocoaWindow(app, "Orbit · Krypton Torrent Manager", 1120, 800)
    put("window", win)
    cocoaSetWindowMinSize(win, 1120, 800)
    cocoaSetWindowBg(win, cocoaRGB(22, 16, 31))
    msg_1(win, "setAppearance:", msg_1(cls("NSAppearance"), "appearanceNamed:", nsString("NSAppearanceNameDarkAqua")))
    let bar = cocoaMenuBar(app)
    let menu = cocoaMenuAdd(bar, "Orbit")
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
    cocoaClassAddMethod(delegateClass, "numberOfRowsInTableView:", funcptr(dsRows), "q@:@")
    cocoaClassAddMethod(delegateClass, "tableView:objectValueForTableColumn:row:", funcptr(dsValue), "@@:@@q")
    cocoaClassRegister(delegateClass)
    let delegate = cocoaNew(delegateClass)
    put("delegate", delegate)
    msg_1(win, "setDelegate:", delegate)
    msg_1(app, "setDelegate:", delegate)
    labelH(win, "ORBIT", 24, 746, 160, 34)
    labelH(win, "Local transfers. Native Krypton.", 190, 751, 500, 24)
    put("status", labelH(win, "Starting…", 24, 714, 1070, 26))
    cocoaSetTextColor(get("status"), cocoaRGB(183, 255, 60))
    // Paste field belongs to the Add Link dialog, never the main window.
    put("source", msg_frame(msg(cls("NSTextField"), "alloc"), "initWithFrame:", 0, 0, 470, 28))
    cocoaSetPlaceholder(get("source"), "magnet:?xt=… or https://…/download.torrent")
    put("pending", cocoaArray())
    buttonH(win, "Add Link…", 24, 666, 160, "preview", funcptr(onPreview))
    buttonH(win, "Open Torrent…", 196, 666, 170, "open", funcptr(chooseTorrent))
    buttonH(win, "Use Orbit for Browser Links…", 378, 666, 260, "browser", funcptr(useBrowserLinks))
    labelH(win, "Preview first. Choose files. Download.", 660, 671, 430, 24)
    put("table", cocoaTable(win, 24, 384, 1066, 264))
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
    cocoaSetAutoresizing(msg(table, "enclosingScrollView"), 18)
    buttonH(win, "Pause", 24, 344, 90, "pause", funcptr(pauseSelected))
    buttonH(win, "Resume", 122, 344, 90, "resume", funcptr(resumeSelected))
    buttonH(win, "Pause All", 220, 344, 100, "pauseall", funcptr(pauseAll))
    buttonH(win, "Resume All", 328, 344, 100, "resumeall", funcptr(resumeAll))
    buttonH(win, "Show in Finder", 436, 344, 150, "folder", funcptr(openFolder))
    buttonH(win, "Copy Magnet", 594, 344, 140, "magnet", funcptr(copyMagnet))
    buttonH(win, "Remove…", 742, 344, 120, "remove", funcptr(removeSelected))
    put("previewtitle", labelH(win, "Preview a torrent to choose files before downloading.", 24, 310, 1060, 24))
    put("files", cocoaTable(win, 24, 192, 720, 110))
    cocoaSetDataSource(get("files"), delegate)
    msg_1(get("files"), "setHeaderView:", 0)
    buttonH(win, "Toggle Selected File", 764, 266, 326, "togglefile", funcptr(toggleFile))
    put("paused", cocoaCheckbox(win, "Start paused", 764, 233, 180, 26))
    let policies = cocoaArray()
    cocoaArrayAdd(policies, nsString("Stop when complete"))
    cocoaArrayAdd(policies, nsString("Keep seeding"))
    cocoaArrayAdd(policies, nsString("Remove when complete (keep files)"))
    put("policy", popup(win, 764, 198, 326, policies))
    put("destination", cocoaTextField(win, 24, 150, 720, 28))
    buttonH(win, "Choose Folder…", 764, 148, 148, "destination", funcptr(chooseDestination))
    put("add", buttonH(win, "Add Transfer", 920, 148, 170, "add", funcptr(addPreview)))
    cocoaSetEnabled(get("add"), 0)
    let modes = cocoaArray()
    cocoaArrayAdd(modes, nsString("Offline"))
    cocoaArrayAdd(modes, nsString("Download Only"))
    cocoaArrayAdd(modes, nsString("Standard (sharing)"))
    put("mode", popup(win, 24, 103, 210, modes))
    put("dht", cocoaCheckbox(win, "DHT", 248, 103, 80, 26))
    put("remember", cocoaCheckbox(win, "Remember mode", 332, 103, 170, 26))
    labelH(win, "Down / Up KiB/s", 520, 106, 150, 22)
    put("down", cocoaTextField(win, 674, 103, 85, 26))
    put("up", cocoaTextField(win, 770, 103, 85, 26))
    buttonH(win, "Apply Network Settings", 866, 101, 224, "configure", funcptr(applySettings))
    labelH(win, "Offline until you choose a mode. DHT / PEX expose your IP; PEX is engine-managed. 0 KiB/s = unlimited.", 24, 62, 1066, 24)
    labelH(win, "Closing this window quits Orbit and stops transfers. Settings and resume data stay on this Mac.", 24, 28, 1066, 24)
    if startCore() != 1 { cocoaAlert("Orbit could not start", "The bundled torrent engine could not launch.") exit(1) }
    cocoaShow(win, app)
    cocoaFinishLaunching(app)
    let frames = 0
    let smoke = arg("0") == "--smoke"
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
            kp("PASS: Objective-K UI and torrent engine bridge")
            put("quitting", cocoaNumber(1))
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
