// WebSocket client for the game server admin API (/api/admin/v1/live).
// All live data (player positions, connects/disconnects) comes from here.
// Jazz is used separately only for collaborative admin annotations.

export interface AdminPlayer {
  player_id: number;
  x: number;
  y: number;
  z: number;
  color: number;
}

export type AdminEvent =
  | { type: "snapshot"; players: AdminPlayer[]; tick: number }
  | { type: "player_joined"; player_id: number; color: number; x: number; y: number; z: number }
  | { type: "player_left"; player_id: number }
  | { type: "tick"; players: AdminPlayer[]; tick: number };

export type EventHandler = (event: AdminEvent) => void;

export class GameAdminWS {
  private ws: WebSocket | null = null;
  private handlers: Set<EventHandler> = new Set();
  private token: string;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private closed = false;

  constructor(token: string) {
    this.token = token;
  }

  connect() {
    this.closed = false;
    this._open();
  }

  private _open() {
    const proto = location.protocol === "https:" ? "wss" : "ws";
    const url = `${proto}://${location.host}/api/admin/v1/live?token=${encodeURIComponent(this.token)}`;
    this.ws = new WebSocket(url);

    this.ws.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data) as AdminEvent;
        this.handlers.forEach((h) => h(event));
      } catch {
        // ignore malformed frames
      }
    };

    this.ws.onclose = () => {
      if (!this.closed) {
        this.reconnectTimer = setTimeout(() => this._open(), 3000);
      }
    };
  }

  disconnect() {
    this.closed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.ws?.close();
    this.ws = null;
  }

  on(handler: EventHandler) {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }
}

// Fetch the current REST snapshot (used for initial load without WS).
export async function fetchSnapshot(token: string): Promise<AdminPlayer[]> {
  const res = await fetch("/api/admin/v1/players", {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) throw new Error(`${res.status}`);
  const data = await res.json();
  return data.players ?? [];
}

// Color palette mirrored from game_protocol PLAYER_PALETTE
const PALETTE = [
  "#f28da6", "#f2a659", "#f2d966", "#8dd980",
  "#66ccbf", "#66a6f2", "#a68ce6", "#d980d9",
];

export function playerColor(index: number): string {
  return PALETTE[index % PALETTE.length];
}
