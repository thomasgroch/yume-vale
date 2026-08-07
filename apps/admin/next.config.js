/** @type {import('next').NextConfig} */
const nextConfig = {
  output: "standalone",
  // The ingress strips /admin before sending to this service, so no basePath needed.
  // API calls use relative paths: /api/admin/... which Traefik proxies to yume-server:5003
};

module.exports = nextConfig;
