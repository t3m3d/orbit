export type TorrentStatus = "downloading" | "seeding" | "paused" | "complete";

export type OrbitTorrent = {
  id: number;
  engineId?: string;
  source?: "demo" | "node";
  name: string;
  meta: string;
  size: string;
  progress: number;
  status: TorrentStatus;
  down: string;
  up: string;
  peers: number;
  eta: string;
  kind: string;
  accent: string;
  hash: string;
  outputFolder?: string;
};

export type NodeConfig = { endpoint: string; token: string };

type RqbitStats = {
  state?: unknown;
  progress_bytes?: number;
  uploaded_bytes?: number;
  total_bytes?: number;
  finished?: boolean;
  live?: {
    download_speed?: unknown;
    upload_speed?: unknown;
    time_remaining?: unknown;
    snapshot?: Record<string, unknown>;
  };
};

type RqbitTorrent = {
  id?: number;
  info_hash?: string;
  name?: string | null;
  output_folder?: string;
  files?: Array<{ name?: string; length?: number; included?: boolean }>;
  stats?: RqbitStats;
};

function endpoint(config: NodeConfig) {
  const value = config.endpoint.trim().replace(/\/+$/, "");
  if (!/^https?:\/\//i.test(value)) throw new Error("Use an http:// or https:// node address.");
  return value;
}

function headers(config: NodeConfig, contentType?: string) {
  const result: Record<string, string> = {};
  if (config.token.trim()) result.Authorization = `Bearer ${config.token.trim()}`;
  if (contentType) result["Content-Type"] = contentType;
  return result;
}

async function request(config: NodeConfig, path: string, init?: RequestInit) {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 6500);
  try {
    const response = await fetch(`${endpoint(config)}${path}`, { ...init, signal: controller.signal });
    if (!response.ok) throw new Error(`Node returned ${response.status}.`);
    return response;
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") throw new Error("The node did not respond in time.");
    if (error instanceof TypeError) throw new Error("Orbit could not reach that node. Check the address and CORS settings.");
    throw error;
  } finally {
    window.clearTimeout(timeout);
  }
}

function scalar(value: unknown): number {
  if (typeof value === "number") return value;
  if (typeof value === "string") return Number.parseFloat(value) || 0;
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    for (const key of ["bytes_per_second", "bytes", "value", "bps"]) {
      if (typeof record[key] === "number") return record[key] as number;
    }
  }
  return 0;
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "Calculating";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** unit;
  return `${value >= 10 || unit === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

function formatSpeed(value: unknown) {
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    if (typeof record.human_readable === "string") return record.human_readable;
  }
  const speed = scalar(value);
  return speed > 0 ? `${formatBytes(speed)}/s` : "—";
}

function formatEta(value: unknown) {
  if (typeof value === "string" && value) return value;
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    if (typeof record.human_readable === "string") return record.human_readable;
    if (typeof record.secs === "number") return `${Math.max(1, Math.round(record.secs / 60))} min`;
  }
  return "Calculating";
}

function peerCount(snapshot?: Record<string, unknown>) {
  if (!snapshot) return 0;
  for (const key of ["live_peers", "peers", "connected_peers", "peer_count"]) {
    const value = snapshot[key];
    if (typeof value === "number") return value;
    if (Array.isArray(value)) return value.length;
  }
  return 0;
}

function kindFor(name: string) {
  const extension = name.split(".").pop()?.slice(0, 4).toUpperCase();
  return extension && extension.length <= 4 ? extension : "TOR";
}

function mapTorrent(torrent: RqbitTorrent, index: number): OrbitTorrent {
  const stats = torrent.stats;
  const total = stats?.total_bytes || torrent.files?.reduce((sum, file) => sum + (file.included === false ? 0 : file.length || 0), 0) || 0;
  const progressBytes = stats?.progress_bytes || 0;
  const progress = stats?.finished ? 100 : total > 0 ? Math.min(100, Math.round((progressBytes / total) * 100)) : 0;
  const state = JSON.stringify(stats?.state ?? "").toLowerCase();
  const paused = state.includes("pause");
  const uploadSpeed = scalar(stats?.live?.upload_speed);
  const status: TorrentStatus = paused ? "paused" : stats?.finished ? (uploadSpeed > 0 ? "seeding" : "complete") : "downloading";
  const name = torrent.name || `Torrent ${torrent.id ?? index + 1}`;
  const files = torrent.files?.filter((file) => file.included !== false).length || 0;

  return {
    id: 100000 + (torrent.id ?? index),
    engineId: String(torrent.id ?? index),
    source: "node",
    name,
    meta: `${files || "Metadata pending"}${files ? ` file${files === 1 ? "" : "s"}` : ""} · rqbit node`,
    size: formatBytes(total),
    progress,
    status,
    down: formatSpeed(stats?.live?.download_speed),
    up: formatSpeed(stats?.live?.upload_speed),
    peers: peerCount(stats?.live?.snapshot),
    eta: status === "paused" ? "Paused" : stats?.finished ? (status === "seeding" ? "Seeding" : "Complete") : formatEta(stats?.live?.time_remaining),
    kind: kindFor(name),
    accent: ["violet", "orange", "lime", "blue", "pink"][index % 5],
    hash: torrent.info_hash || "INFO HASH PENDING",
    outputFolder: torrent.output_folder,
  };
}

export async function testNode(config: NodeConfig) {
  const response = await request(config, "/", { headers: headers(config) });
  const data = (await response.json()) as { server?: string; apis?: unknown };
  if (data.server !== "rqbit" && !data.apis) throw new Error("This does not look like an rqbit node.");
  return true;
}

export async function listNodeTorrents(config: NodeConfig) {
  const response = await request(config, "/torrents?with_stats=true", { headers: headers(config) });
  const data = (await response.json()) as { torrents?: RqbitTorrent[] };
  return (data.torrents || []).map(mapTorrent);
}

export async function addMagnetToNode(config: NodeConfig, magnet: string) {
  await request(config, "/torrents", { method: "POST", headers: headers(config, "text/plain"), body: magnet });
}

export async function addTorrentFileToNode(config: NodeConfig, file: File) {
  await request(config, "/torrents", { method: "POST", headers: headers(config, "application/x-bittorrent"), body: file });
}

export async function runNodeAction(config: NodeConfig, engineId: string, action: "pause" | "start" | "forget") {
  await request(config, `/torrents/${encodeURIComponent(engineId)}/${action}`, { method: "POST", headers: headers(config) });
}