import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Yume Vale Admin",
  description: "Admin panel — live game server observer",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="pt">
      <body>{children}</body>
    </html>
  );
}
