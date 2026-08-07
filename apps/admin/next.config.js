/** @type {import('next').NextConfig} */
const path = require("path");
const nextConfig = {
  output: "standalone",
  // Fix: Next.js workspace root detection when multiple lockfiles exist.
  outputFileTracingRoot: path.join(__dirname, "../../"),
  // The ingress strips /admin before sending to this service, so no basePath needed.
  // API calls use relative paths: /api/admin/... which Traefik proxies to yume-server:5003
};

module.exports = nextConfig;
