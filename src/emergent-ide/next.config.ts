import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  distDir: process.env.EMERGENT_NEXT_DIST_DIR ?? ".next",
  cacheComponents: true,
  partialPrefetching: true,
  experimental: {
    // The persisted dev graph has resumed into a rebuild loop on restart.
    turbopackFileSystemCacheForDev: false,
  },
  turbopack: {
    rules: {
      "*.css": {
        loaders: ["@tailwindcss/turbopack"],
        as: "*.css",
      },
    },
  },
};

export default nextConfig;
