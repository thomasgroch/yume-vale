"use client";
import { AdminPlayer, playerColor } from "@/lib/game-ws";

// The world arena sits roughly within ±40 units on X and Z.
const WORLD_SIZE = 80;
const MAP_PX = 320;

function worldToCanvas(v: number): number {
  return ((v + WORLD_SIZE / 2) / WORLD_SIZE) * MAP_PX;
}

interface Props {
  players: AdminPlayer[];
  selected: number | null;
  onSelect: (id: number) => void;
}

export default function LiveMap({ players, selected, onSelect }: Props) {
  return (
    <svg
      width={MAP_PX}
      height={MAP_PX}
      style={{ border: "1px solid #e0e0e0", borderRadius: 8, background: "#f8fdf4", display: "block" }}
    >
      {/* Grid */}
      {[0, 0.25, 0.5, 0.75, 1].map((t) => (
        <g key={t}>
          <line
            x1={t * MAP_PX} y1={0} x2={t * MAP_PX} y2={MAP_PX}
            stroke="#e0e0e0" strokeWidth={0.5}
          />
          <line
            x1={0} y1={t * MAP_PX} x2={MAP_PX} y2={t * MAP_PX}
            stroke="#e0e0e0" strokeWidth={0.5}
          />
        </g>
      ))}

      {/* Players */}
      {players.map((p) => {
        const cx = worldToCanvas(p.x);
        const cy = worldToCanvas(p.z);
        const isSelected = p.player_id === selected;
        return (
          <g key={p.player_id} style={{ cursor: "pointer" }} onClick={() => onSelect(p.player_id)}>
            {isSelected && (
              <circle cx={cx} cy={cy} r={10} fill="none" stroke="#3b82f6" strokeWidth={2} />
            )}
            <circle
              cx={cx} cy={cy} r={6}
              fill={playerColor(p.color)}
              stroke="#fff"
              strokeWidth={1.5}
            />
            <text
              x={cx} y={cy - 9}
              textAnchor="middle"
              fontSize={9}
              fill="#444"
              fontFamily="monospace"
            >
              #{p.player_id}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
