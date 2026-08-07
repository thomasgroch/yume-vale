"use client";
import { AdminPlayer, playerColor } from "@/lib/game-ws";

interface Props {
  players: AdminPlayer[];
  selected: number | null;
  onSelect: (id: number) => void;
}

export default function PlayerList({ players, selected, onSelect }: Props) {
  if (players.length === 0) {
    return (
      <div style={{ padding: 16, color: "#888", fontSize: 13 }}>
        Nenhum jogador online.
      </div>
    );
  }

  return (
    <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
      {players.map((p) => (
        <li
          key={p.player_id}
          onClick={() => onSelect(p.player_id)}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "10px 16px",
            cursor: "pointer",
            background: selected === p.player_id ? "#e8f0ff" : "transparent",
            borderLeft: selected === p.player_id ? "3px solid #3b82f6" : "3px solid transparent",
          }}
        >
          <span
            style={{
              width: 12,
              height: 12,
              borderRadius: "50%",
              background: playerColor(p.color),
              flexShrink: 0,
            }}
          />
          <span style={{ fontFamily: "monospace", fontSize: 13 }}>
            #{p.player_id}
          </span>
          <span style={{ fontSize: 12, color: "#666", marginLeft: "auto" }}>
            ({p.x.toFixed(1)}, {p.z.toFixed(1)})
          </span>
        </li>
      ))}
    </ul>
  );
}
