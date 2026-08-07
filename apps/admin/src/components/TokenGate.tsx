"use client";
import { useState, useEffect } from "react";

const KEY = "yume_admin_token";

interface Props {
  children: (token: string) => React.ReactNode;
}

export default function TokenGate({ children }: Props) {
  const [token, setToken] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    const stored = sessionStorage.getItem(KEY);
    if (stored) setToken(stored);
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    try {
      const res = await fetch("/api/admin/v1/players", {
        headers: { Authorization: `Bearer ${input}` },
      });
      if (res.ok) {
        sessionStorage.setItem(KEY, input);
        setToken(input);
      } else {
        setError("Token inválido.");
      }
    } catch {
      setError("Servidor inacessível.");
    }
  }

  if (token) return <>{children(token)}</>;

  return (
    <div style={{ display: "flex", height: "100vh", alignItems: "center", justifyContent: "center" }}>
      <form onSubmit={submit} style={{ display: "flex", flexDirection: "column", gap: 12, width: 300 }}>
        <h2 style={{ margin: 0 }}>Yume Vale — Admin</h2>
        <input
          type="password"
          placeholder="YUME_ADMIN_TOKEN"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          style={{ padding: "8px 12px", fontSize: 14, borderRadius: 6, border: "1px solid #ccc" }}
        />
        {error && <span style={{ color: "red", fontSize: 13 }}>{error}</span>}
        <button type="submit" style={{ padding: "8px 0", borderRadius: 6, cursor: "pointer" }}>
          Entrar
        </button>
      </form>
    </div>
  );
}
