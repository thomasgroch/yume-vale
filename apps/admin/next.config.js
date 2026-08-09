/** @type {import('next').NextConfig} */
const path = require("path");
const nextConfig = {
  output: "standalone",
  // Fix: Next.js workspace root detection when multiple lockfiles exist.
  outputFileTracingRoot: path.join(__dirname, "../../"),
  // Served under /admin — basePath makes Next.js prefix its own page routes
  // and static assets (/admin/_next/...) so they don't collide with the
  // root ingress route (yume-client) which has no knowledge of /_next paths.
  basePath: "/admin",
  // API calls use absolute paths: /api/admin/... which Traefik proxies to
  // yume-server:5003 directly — NOT affected by basePath (that only rewrites
  // this app's own routes/assets, not arbitrary fetch() URLs).
};

module.exports = nextConfig;
