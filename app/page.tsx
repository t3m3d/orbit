"use client";

import { useEffect, useMemo, useRef, useState } from "react";

import {
  addMagnetToNode,
  addTorrentFileToNode,
  listNodeTorrents,
  runNodeAction,
  testNode,
  type NodeConfig,
  type OrbitTorrent,
  type TorrentStatus,
} from "./orbit-node";

const starterTorrents: OrbitTorrent[] = [
  {
    id: 1,
    name: "Aurora Linux 4.2",
    meta: "Operating system · 1 file",
    size: "4.8 GB",
    progress: 68,
    status: "downloading",
    down: "18.4 MB/s",
    up: "1.2 MB/s",
    peers: 86,
    eta: "7 min",
    kind: "ISO",
    accent: "violet",
    hash: "7D2A 09BE C481 32AF 918B 44AC 6F38 3E20",
  },
  {
    id: 2,
    name: "Open Images Archive 06",
    meta: "Photography · 2,481 files",
    size: "12.4 GB",
    progress: 100,
    status: "seeding",
    down: "—",
    up: "842 KB/s",
    peers: 34,
    eta: "Seeding",
    kind: "ZIP",
    accent: "orange",
    hash: "AA1F E0C3 12DD 84B1 9A82 01E9 D11A 7F49",
  },
  {
    id: 3,
    name: "Field Recordings Vol. 3",
    meta: "Audio · 42 files",
    size: "2.1 GB",
    progress: 43,
    status: "downloading",
    down: "6.2 MB/s",
    up: "318 KB/s",
    peers: 19,
    eta: "11 min",
    kind: "WAV",
    accent: "lime",
    hash: "0C9E A7F3 8D1C 6A24 B5FE 75D0 0344 71C1",
  },
  {
    id: 4,
    name: "Blender Studio Assets",
    meta: "Creative commons · 318 files",
    size: "8.7 GB",
    progress: 100,
    status: "complete",
    down: "—",
    up: "—",
    peers: 0,
    eta: "Complete",
    kind: "3D",
    accent: "blue",
    hash: "64DC 2A10 FFFE B55D 21C8 BCAA 0973 688A",
  },
  {
    id: 5,
    name: "Public Domain Cinema Pack",
    meta: "Video · 8 files",
    size: "17.3 GB",
    progress: 16,
    status: "paused",
    down: "—",
    up: "—",
    peers: 12,
    eta: "Paused",
    kind: "MP4",
    accent: "pink",
    hash: "F234 BC87 8911 D420 5DA9 3AC8 E1A0 4B92",
  },
];

const statusCopy: Record<TorrentStatus, string> = {
  downloading: "Downloading",
  seeding: "Seeding",
  paused: "Paused",
  complete: "Complete",
};

export default function Home() {
  const [torrents, setTorrents] = useState<OrbitTorrent[]>(starterTorrents);
  const [selectedId, setSelectedId] = useState(1);
  const [filter, setFilter] = useState<"all" | TorrentStatus>("all");
  const [search, setSearch] = useState("");
  const [addOpen, setAddOpen] = useState(false);
  const [magnet, setMagnet] = useState("");
  const [newName, setNewName] = useState("");
  const [toast, setToast] = useState("");
  const [nodeOpen, setNodeOpen] = useState(false);
  const [nodeStatus, setNodeStatus] = useState<"demo" | "connecting" | "connected" | "error">("demo");
  const [nodeConfig, setNodeConfig] = useState<NodeConfig>({ endpoint: "http://127.0.0.1:3030", token: "" });
  const [nodeError, setNodeError] = useState("");
  const [syncing, setSyncing] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const saved = window.localStorage.getItem("orbit-torrents");
    if (saved) {
      try {
        setTorrents(JSON.parse(saved));
      } catch {
        window.localStorage.removeItem("orbit-torrents");
      }
    }
  }, []);

  useEffect(() => {
    if (nodeStatus === "demo") window.localStorage.setItem("orbit-torrents", JSON.stringify(torrents));
  }, [nodeStatus, torrents]);

  useEffect(() => {
    const savedNode = window.localStorage.getItem("orbit-node");
    if (!savedNode) return;
    try {
      const config = JSON.parse(savedNode) as NodeConfig;
      setNodeConfig(config);
      void connectNode(config, true);
    } catch {
      window.localStorage.removeItem("orbit-node");
    }
  }, []);

  useEffect(() => {
    if (nodeStatus !== "connected") return;
    const interval = window.setInterval(() => void syncFromNode(nodeConfig, true), 5000);
    return () => window.clearInterval(interval);
  }, [nodeStatus, nodeConfig.endpoint, nodeConfig.token]);

  const selected = torrents.find((torrent) => torrent.id === selectedId) ?? torrents[0];
  const visible = useMemo(
    () =>
      torrents.filter((torrent) => {
        const matchesFilter = filter === "all" || torrent.status === filter;
        const matchesSearch = torrent.name.toLowerCase().includes(search.toLowerCase());
        return matchesFilter && matchesSearch;
      }),
    [filter, search, torrents],
  );

  const downloading = torrents.filter((torrent) => torrent.status === "downloading").length;
  const seeding = torrents.filter((torrent) => torrent.status === "seeding").length;
  const completed = torrents.filter((torrent) => torrent.status === "complete").length;

  function flash(message: string) {
    setToast(message);
    window.setTimeout(() => setToast(""), 2400);
  }

  async function syncFromNode(config = nodeConfig, quiet = false) {
    if (!quiet) setSyncing(true);
    try {
      const liveTorrents = await listNodeTorrents(config);
      setTorrents(liveTorrents);
      setNodeStatus("connected");
      setNodeError("");
      if (liveTorrents.length && !liveTorrents.some((torrent) => torrent.id === selectedId)) setSelectedId(liveTorrents[0].id);
    } catch (error) {
      setNodeStatus("error");
      setNodeError(error instanceof Error ? error.message : "Could not sync the node.");
    } finally {
      setSyncing(false);
    }
  }

  async function connectNode(config = nodeConfig, quiet = false) {
    setNodeStatus("connecting");
    setNodeError("");
    try {
      await testNode(config);
      window.localStorage.setItem("orbit-node", JSON.stringify(config));
      setNodeConfig(config);
      await syncFromNode(config, quiet);
      setNodeOpen(false);
      if (!quiet) flash("rqbit node connected");
    } catch (error) {
      setNodeStatus("error");
      setNodeError(error instanceof Error ? error.message : "Could not connect to the node.");
    }
  }

  function disconnectNode() {
    window.localStorage.removeItem("orbit-node");
    setNodeStatus("demo");
    setNodeError("");
    const saved = window.localStorage.getItem("orbit-torrents");
    setTorrents(saved ? JSON.parse(saved) : starterTorrents);
    setSelectedId(1);
    setNodeOpen(false);
    flash("Switched to demo mode");
  }

  async function toggleTorrent(id: number) {
    const target = torrents.find((torrent) => torrent.id === id);
    if (!target) return;
    if (target.source === "node" && target.engineId) {
      try {
        await runNodeAction(nodeConfig, target.engineId, target.status === "paused" ? "start" : "pause");
        await syncFromNode(nodeConfig, true);
        flash(target.status === "paused" ? "Transfer resumed" : "Transfer paused");
      } catch (error) {
        flash(error instanceof Error ? error.message : "Node action failed");
      }
      return;
    }
    setTorrents((current) => current.map((torrent) => torrent.id === id ? {
      ...torrent,
      status: torrent.status === "paused" ? "downloading" : "paused",
      down: torrent.status === "paused" ? "4.6 MB/s" : "—",
      eta: torrent.status === "paused" ? "18 min" : "Paused",
    } : torrent));
    flash(target.status === "paused" ? "Transfer resumed" : "Transfer paused");
  }

  async function removeTorrent(id: number) {
    const target = torrents.find((torrent) => torrent.id === id);
    if (!target) return;
    if (target.source === "node" && target.engineId) {
      try {
        await runNodeAction(nodeConfig, target.engineId, "forget");
        await syncFromNode(nodeConfig, true);
        flash(`${target.name} removed; files kept`);
      } catch (error) {
        flash(error instanceof Error ? error.message : "Node action failed");
      }
      return;
    }
    setTorrents((current) => current.filter((torrent) => torrent.id !== id));
    const next = torrents.find((torrent) => torrent.id !== id);
    if (next) setSelectedId(next.id);
    flash(`${target.name} removed`);
  }

  async function addTorrent(name?: string) {
    if (nodeStatus === "connected") {
      if (!magnet.trim()) return;
      try {
        await addMagnetToNode(nodeConfig, magnet.trim());
        await syncFromNode(nodeConfig, true);
        setAddOpen(false);
        setMagnet("");
        setNewName("");
        flash("Torrent added to the live node");
      } catch (error) {
        flash(error instanceof Error ? error.message : "Could not add the torrent");
      }
      return;
    }
    const cleanedName = name || newName.trim() || "New magnet transfer";
    const nextId = Math.max(0, ...torrents.map((torrent) => torrent.id)) + 1;
    const nextTorrent: OrbitTorrent = {
      id: nextId,
      source: "demo",
      name: cleanedName.replace(/\.torrent$/i, ""),
      meta: "New transfer · Metadata pending",
      size: "Calculating",
      progress: 0,
      status: "downloading",
      down: "Connecting",
      up: "—",
      peers: 0,
      eta: "Finding peers",
      kind: name ? "TOR" : "MAG",
      accent: "violet",
      hash: "MAGNET LINK ADDED · INFO HASH PENDING",
    };
    setTorrents((current) => [nextTorrent, ...current]);
    setSelectedId(nextId);
    setFilter("all");
    setAddOpen(false);
    setMagnet("");
    setNewName("");
    flash("Demo transfer added");
  }

  async function addTorrentFile(file: File) {
    if (nodeStatus !== "connected") {
      await addTorrent(file.name);
      return;
    }
    try {
      await addTorrentFileToNode(nodeConfig, file);
      await syncFromNode(nodeConfig, true);
      setAddOpen(false);
      flash("Torrent file added to the live node");
    } catch (error) {
      flash(error instanceof Error ? error.message : "Could not add the torrent file");
    }
  }
  function chooseFilter(nextFilter: "all" | TorrentStatus) {
    setFilter(nextFilter);
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand" aria-label="Orbit home">
          <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
          <span>ORBIT</span>
          <b>BETA</b>
        </div>

        <nav className="side-nav" aria-label="Torrent filters">
          <p>LIBRARY</p>
          <button className={filter === "all" ? "active" : ""} onClick={() => chooseFilter("all")}>
            <span className="nav-icon">⌁</span> All torrents <em>{torrents.length}</em>
          </button>
          <button className={filter === "downloading" ? "active" : ""} onClick={() => chooseFilter("downloading")}>
            <span className="nav-icon">↓</span> Downloading <em>{downloading}</em>
          </button>
          <button className={filter === "seeding" ? "active" : ""} onClick={() => chooseFilter("seeding")}>
            <span className="nav-icon">↑</span> Seeding <em>{seeding}</em>
          </button>
          <button className={filter === "complete" ? "active" : ""} onClick={() => chooseFilter("complete")}>
            <span className="nav-icon">✓</span> Completed <em>{completed}</em>
          </button>
          <p>COLLECTIONS</p>
          <button><span className="label-dot purple" /> Linux</button>
          <button><span className="label-dot orange" /> Media</button>
          <button><span className="label-dot green" /> Work</button>
        </nav>

        <button className={`node-card ${nodeStatus}`} onClick={() => setNodeOpen(true)}>
          <div className="node-heading"><span className={`pulse-dot ${nodeStatus === "connected" ? "" : "offline"}`} /> TORRENT NODE</div>
          <strong>{nodeStatus === "connected" ? "rqbit node" : "Connect a node"}</strong>
          <small>{nodeStatus === "connected" ? "Live queue · Synced" : nodeStatus === "error" ? "Connection needs attention" : "Run real transfers here"}</small>
          <div className="node-meter"><i style={{ width: nodeStatus === "connected" ? "100%" : "18%" }} /></div>
          <div className="node-storage"><span>Mode</span><b>{nodeStatus === "connected" ? "Live engine" : "Demo data"}</b></div>
        </button>

        <div className="profile">
          <span>BK</span>
          <div><strong>Brian&apos;s Orbit</strong><small>Local-first workspace</small></div>
          <button aria-label="Open settings">•••</button>
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div className="mobile-brand"><span className="brand-mark"><i /><i /><i /></span> ORBIT</div>
          <label className="search-box">
            <span aria-hidden="true">⌕</span>
            <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search transfers" aria-label="Search transfers" />
            <kbd>⌘ K</kbd>
          </label>
          <div className="top-actions">
            <button className="icon-button" aria-label="Open activity">◔</button>
            <button className="add-button" onClick={() => setAddOpen(true)}><span>＋</span> {nodeStatus === "connected" ? "Add to node" : "Add torrent"}</button>
          </div>
        </header>

        <div className="dashboard">
          <div className="content-column">
            <div className="page-heading">
              <div>
                <p>LIBRARY / <span>{filter === "all" ? "ALL TORRENTS" : statusCopy[filter as TorrentStatus].toUpperCase()}</span></p>
                <h1>Good evening, Brian.</h1>
                <h2>Your queue is moving smoothly.</h2>
              </div>
              <button className={`global-control ${nodeStatus}`} onClick={() => nodeStatus === "connected" ? void syncFromNode() : setNodeOpen(true)}>
                <span className={`live-light ${nodeStatus === "connected" ? "" : "offline"}`} />
                <div><strong>{nodeStatus === "connected" ? "Live node connected" : "Orbit demo mode"}</strong><small>{syncing ? "Syncing now…" : nodeStatus === "connected" ? nodeConfig.endpoint : "Connect rqbit to enable downloads"}</small></div>
              </button>
            </div>

            <section className="stats-grid" aria-label="Transfer statistics">
              <article><span className="stat-icon violet">↓</span><div><small>DOWNLOAD</small><strong>24.6 <em>MB/s</em></strong></div><b>+8%</b></article>
              <article><span className="stat-icon orange">↑</span><div><small>UPLOAD</small><strong>2.3 <em>MB/s</em></strong></div><b>steady</b></article>
              <article><span className="stat-icon lime">⇄</span><div><small>RATIO</small><strong>1.42</strong></div><b>healthy</b></article>
              <article><span className="stat-icon blue">◉</span><div><small>PEERS</small><strong>139</strong></div><b>3 swarms</b></article>
            </section>

            {selected && (
              <section className="focus-card">
                <div className={`file-tile ${selected.accent}`}><span>{selected.kind}</span><i>↙</i></div>
                <div className="focus-copy">
                  <div className="eyebrow"><span className="moving-dot" /> NOW TRANSFERRING</div>
                  <h3>{selected.name}</h3>
                  <p>{selected.meta} <span>•</span> {selected.size}</p>
                  <div className="focus-progress"><i style={{ width: `${selected.progress}%` }} /></div>
                  <div className="progress-caption"><strong>{selected.progress}%</strong><span>{selected.eta}</span></div>
                </div>
                <div className="focus-speeds">
                  <div><span>↓</span><strong>{selected.down}</strong><small>DOWN</small></div>
                  <div><span>↑</span><strong>{selected.up}</strong><small>UP</small></div>
                </div>
                <button className="round-control" onClick={() => toggleTorrent(selected.id)} aria-label={selected.status === "paused" ? "Resume selected torrent" : "Pause selected torrent"}>
                  {selected.status === "paused" ? "▶" : "Ⅱ"}
                </button>
              </section>
            )}

            <section className="transfer-section">
              <div className="section-heading">
                <div><h3>Transfers</h3><span>{visible.length} shown</span></div>
                <div className="view-actions"><button className="selected" aria-label="List view">☷</button><button aria-label="Grid view">⊞</button></div>
              </div>

              <div className="transfer-list">
                <div className="table-head"><span>NAME</span><span>SIZE</span><span>PROGRESS</span><span>SPEED</span><span>PEERS</span><span /></div>
                {visible.map((torrent) => (
                  <article key={torrent.id} className={`transfer-row ${torrent.id === selectedId ? "selected-row" : ""}`} onClick={() => setSelectedId(torrent.id)}>
                    <div className="torrent-name">
                      <span className={`mini-file ${torrent.accent}`}>{torrent.kind}</span>
                      <div><strong>{torrent.name}</strong><small>{torrent.meta}</small></div>
                    </div>
                    <span className="size-cell">{torrent.size}</span>
                    <div className="row-progress">
                      <div><i style={{ width: `${torrent.progress}%` }} /></div>
                      <span>{torrent.progress}%</span>
                    </div>
                    <div className="speed-cell">
                      <span className={torrent.status === "downloading" ? "speed-live" : ""}>{torrent.down}</span>
                      <small>{statusCopy[torrent.status]}</small>
                    </div>
                    <span className="peer-cell">{torrent.peers || "—"}</span>
                    <button className="row-menu" aria-label={`More options for ${torrent.name}`} onClick={(event) => { event.stopPropagation(); removeTorrent(torrent.id); }}>×</button>
                  </article>
                ))}
                {visible.length === 0 && <div className="empty-state"><strong>No transfers found</strong><span>Try another search or filter.</span></div>}
              </div>
            </section>
          </div>

          {selected && (
            <aside className="details-panel">
              <div className="details-head"><span>TORRENT DETAILS</span><button aria-label="Close details">×</button></div>
              <div className={`details-art ${selected.accent}`}><b>{selected.kind}</b><span>ORBIT TRANSFER</span></div>
              <h3>{selected.name}</h3>
              <p>{selected.meta}</p>
              <div className="detail-status"><span className={selected.status === "paused" ? "paused" : ""} />{statusCopy[selected.status]}</div>
              <div className="detail-rule" />
              <dl>
                <div><dt>Downloaded</dt><dd>{selected.progress}% of {selected.size}</dd></div>
                <div><dt>Time remaining</dt><dd>{selected.eta}</dd></div>
                <div><dt>Availability</dt><dd>8.43</dd></div>
                <div><dt>Ratio</dt><dd>0.64</dd></div>
              </dl>
              <div className="pieces-head"><span>PIECES</span><b>1,284 / 1,888</b></div>
              <div className="pieces" aria-label="Piece availability map">
                {Array.from({ length: 72 }).map((_, index) => <i key={index} className={index < Math.round(selected.progress * 0.72) ? "done" : index % 7 === 0 ? "active" : ""} />)}
              </div>
              <div className="detail-rule" />
              <dl className="path-list">
                <div><dt>Save to</dt><dd>{selected.outputFolder || "Orbit / Downloads"}</dd></div>
                <div><dt>Info hash</dt><dd>{selected.hash}</dd></div>
              </dl>
              <button className="outline-button" onClick={() => toggleTorrent(selected.id)}>{selected.status === "paused" ? "Resume transfer" : "Pause transfer"}</button>
            </aside>
          )}
        </div>
      </section>

      <nav className="mobile-nav" aria-label="Mobile navigation">
        <button className="active"><span>⌁</span>Transfers</button>
        <button><span>◉</span>Activity</button>
        <button className="mobile-add" onClick={() => setAddOpen(true)} aria-label="Add torrent">＋</button>
        <button onClick={() => setNodeOpen(true)}><span>⌂</span>Node</button>
        <button><span>⚙</span>Settings</button>
      </nav>

      {addOpen && (
        <div className="modal-backdrop" onMouseDown={() => setAddOpen(false)}>
          <section className="add-modal" role="dialog" aria-modal="true" aria-labelledby="add-title" onMouseDown={(event) => event.stopPropagation()}>
            <button className="modal-close" onClick={() => setAddOpen(false)} aria-label="Close add torrent dialog">×</button>
            <span className="modal-mark">↙</span>
            <p>NEW TRANSFER</p>
            <h2 id="add-title">{nodeStatus === "connected" ? "Send it to your node." : "Bring something into Orbit."}</h2>
            <label>Name <span>OPTIONAL</span><input value={newName} onChange={(event) => setNewName(event.target.value)} placeholder="Give this transfer a name" /></label>
            <label>Magnet link <textarea value={magnet} onChange={(event) => setMagnet(event.target.value)} placeholder="magnet:?xt=urn:btih:…" rows={3} /></label>
            <div className="or-divider"><span>OR</span></div>
            <input ref={fileInput} type="file" accept=".torrent,application/x-bittorrent" hidden onChange={(event) => event.target.files?.[0] && void addTorrentFile(event.target.files[0])} />
            <button className="file-drop" onClick={() => fileInput.current?.click()}><span>⇧</span><strong>Choose a .torrent file</strong><small>or drop one here</small></button>
            <div className="modal-actions"><button onClick={() => setAddOpen(false)}>Cancel</button><button className="primary" disabled={!magnet.trim()} onClick={() => void addTorrent()}>{nodeStatus === "connected" ? "Add to live node" : "Add demo transfer"}</button></div>
          </section>
        </div>
      )}


      {nodeOpen && (
        <div className="modal-backdrop" onMouseDown={() => setNodeOpen(false)}>
          <section className="add-modal node-modal" role="dialog" aria-modal="true" aria-labelledby="node-title" onMouseDown={(event) => event.stopPropagation()}>
            <button className="modal-close" onClick={() => setNodeOpen(false)} aria-label="Close node dialog">×</button>
            <span className="modal-mark node-mark">⌁</span>
            <p>TORRENT ENGINE</p>
            <h2 id="node-title">Connect your download node.</h2>
            <p className="node-intro">Orbit speaks the rqbit HTTP API. Keep rqbit running on a device that stores your files, then connect to its address.</p>
            <label>Node address<input value={nodeConfig.endpoint} onChange={(event) => setNodeConfig((current) => ({ ...current, endpoint: event.target.value }))} placeholder="http://127.0.0.1:3030" /></label>
            <label>Access token <span>OPTIONAL</span><input type="password" value={nodeConfig.token} onChange={(event) => setNodeConfig((current) => ({ ...current, token: event.target.value }))} placeholder="Bearer token for a secured proxy" /></label>
            {nodeError && <div className="node-error"><span>!</span>{nodeError}</div>}
            <div className="node-guide"><strong>Start a local node</strong><code>rqbit server start ~/Downloads</code><small>Only connect nodes you control. Remote nodes should sit behind HTTPS and authentication.</small></div>
            <div className="modal-actions node-actions">
              {nodeStatus === "connected" && <button className="disconnect" onClick={disconnectNode}>Use demo mode</button>}
              <button onClick={() => setNodeOpen(false)}>Cancel</button>
              <button className="primary" disabled={!nodeConfig.endpoint.trim() || nodeStatus === "connecting"} onClick={() => void connectNode()}>{nodeStatus === "connecting" ? "Connecting…" : "Connect node"}</button>
            </div>
          </section>
        </div>
      )}
      {toast && <div className="toast"><span>✓</span>{toast}</div>}
    </main>
  );
}