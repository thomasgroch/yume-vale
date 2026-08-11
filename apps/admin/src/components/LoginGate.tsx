"use client";
import { useState, useEffect } from "react";
import { login } from "@/lib/game-ws";

export const SESSION_KEY = "yume_admin_session";

interface Props {
  children: (token: string) => React.ReactNode;
}

export default function LoginGate({ children }: Props) {
  const [token, setToken] = useState<string | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    const stored = sessionStorage.getItem(SESSION_KEY);
    if (stored) setToken(stored);
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setSubmitting(true);
    try {
      const sessionToken = await login(username, password);
      sessionStorage.setItem(SESSION_KEY, sessionToken);
      setToken(sessionToken);
    } catch {
      setError("Usuário ou senha inválidos.");
    } finally {
      setSubmitting(false);
    }
  }

  if (token) return <>{children(token)}</>;

  return (
    <div style={{ display: "flex", height: "100vh", alignItems: "center", justifyContent: "center" }}>
      <form onSubmit={submit} style={{ display: "flex", flexDirection: "column", gap: 12, width: 300 }}>
        <h2 style={{ margin: 0 }}>Yume Vale — Admin</h2>
        <input
          type="text"
          placeholder="Usuário"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          style={{ padding: "8px 12px", fontSize: 14, borderRadius: 6, border: "1px solid #ccc" }}
        />
        <input
          type="password"
          placeholder="Senha"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          style={{ padding: "8px 12px", fontSize: 14, borderRadius: 6, border: "1px solid #ccc" }}
        />
        {error && <span style={{ color: "red", fontSize: 13 }}>{error}</span>}
        <button type="submit" disabled={submitting} style={{ padding: "8px 0", borderRadius: 6, cursor: "pointer" }}>
          {submitting ? "Entrando…" : "Entrar"}
        </button>
      </form>
    </div>
  );
}
