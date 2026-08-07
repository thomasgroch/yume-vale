"use client";

import { useEffect, useRef, useState } from "react";
import TokenGate from "@/components/TokenGate";
import PlayerList from "@/components/PlayerList";
import LiveMap from "@/components/LiveMap";
import { AdminPlayer, AdminEvent, GameAdminWS, fetchSnapshot } from "@/lib/game-ws";

// ---------------------------------------------------------------------------
// Dashboard (rendered after successful token auth)
// ---------------------------------------------------------------------------

function Dashboard({ token }: { token: string }) {
  const [players, setPlayers] = useState<AdminPlayer[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [status, setStatus] = useState<"connecting" | "live" | "error">("connecting");
  const [tick, setTick] = useState(0);
  const wsRef = useRef<GameAdminWS | null>(null);

  useEffect(() => {
    let mounted = true;

    // Pre-load via REST while WS connects
    fetchSnapshot(token)
      .then((p) => { if (mounted) setPlayers(p); })
      .catch(() => {});

    const ws = new GameAdminWS(token);
    wsRef.current = ws;

    const unsub = ws.on((event: AdminEvent) => {
      if (!mounted) return;
      setStatus("live");

      if (event.type === "snapshot") {
        setPlayers(event.players);
        setTick(event.tick);
      } else if (event.type === "tick") {
        setPlayers(event.players);
        setTick(event.tick);
      } else if (event.type === "player_joined") {
        setPlayers((prev) => {
          const exists = prev.some((p) => p.player_id === event.player_id);
          if (exists) return prev;
          return [...prev, { player_id: event.player_id, color: event.color, x: event.x, y: event.y, z: event.z }];
        });
      } else if (event.type === "player_left") {
        setPlayers((prev) => prev.filter((p) => p.player_id !== event.player_id));
        setSelected((s) => (s === event.player_id ? null : s));
      }
    });

    ws.connect();

    return () => {
      mounted = false;
      unsub();
      ws.disconnect();
    };
  }, [token]);

  const selectedPlayer = players.find((p) => p.player_id === selected) ?? null;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100vh" }}>
      {/* Header */}
      <header style={{ padding: "12px 20px", borderBottom: "1px solid #e5e7eb", display: "flex", alignItems: "center", gap: 16, background: "#fff" }}>
        <h1 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>Yume Vale — Admin</h1>
        <span style={{ fontSize: 12, color: status === "live" ? "#10b981" : "#f59e0b", fontWeight: 600 }}>
          {status === "live" ? "● live" : "○ conectando…"}
        </span>
        <span style={{ fontSize: 12, color: "#888", marginLeft: "auto" }}>
          {players.length} jogador{players.length !== 1 ? "es" : ""} online · tick {tick}
        </span>
      </header>

      {/* Body */}
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
        {/* Sidebar — player list */}
        <aside style={{ width: 240, borderRight: "1px solid #e5e7eb", overflowY: "auto", background: "#fff" }}>
          <div style={{ padding: "10px 16px 4px", fontSize: 11, color: "#888", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.06em" }}>
            Jogadores
          </div>
          <PlayerList players={players} selected={selected} onSelect={setSelected} />
        </aside>

        {/* Main area */}
        <main style={{ flex: 1, padding: 24, overflowY: "auto", display: "flex", flexDirection: "column", gap: 24 }}>
          <section>
            <h2 style={{ margin: "0 0 12px", fontSize: 15 }}>Mapa ao vivo</h2>
            <LiveMap players={players} selected={selected} onSelect={setSelected} />
          </section>

          {selectedPlayer && (
            <section style={{ background: "#fff", border: "1px solid #e5e7eb", borderRadius: 8, padding: 16 }}>
              <h2 style={{ margin: "0 0 12px", fontSize: 15 }}>Jogador #{selectedPlayer.player_id}</h2>
              <table style={{ borderCollapse: "collapse", fontSize: 13, width: "100%" }}>
                <tbody>
                  {(["x", "y", "z"] as const).map((axis) => (
                    <tr key={axis}>
                      <td style={{ padding: "4px 12px 4px 0", color: "#888", width: 60 }}>{axis.toUpperCase()}</td>
                      <td style={{ fontFamily: "monospace" }}>{selectedPlayer[axis].toFixed(3)}</td>
                    </tr>
                  ))}
                  <tr>
                    <td style={{ padding: "4px 12px 4px 0", color: "#888" }}>Cor</td>
                    <td>
                      <span style={{ display: "inline-block", width: 14, height: 14, borderRadius: "50%", background: `var(--pc-${selectedPlayer.color % 8})`, border: "1px solid #ccc", verticalAlign: "middle", marginRight: 6 }} />
                      índice {selectedPlayer.color}
                    </td>
                  </tr>
                </tbody>
              </table>
            </section>
          )}
        </main>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page root — gate behind admin token
// ---------------------------------------------------------------------------

export default function AdminPage() {
  return <TokenGate>{(token) => <Dashboard token={token} />}</TokenGate>;
}
