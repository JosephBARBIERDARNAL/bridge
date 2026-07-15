import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "BRIDGE_");
  const token = process.env.BRIDGE_DEV_API_TOKEN ?? env.BRIDGE_DEV_API_TOKEN;
  return {
    plugins: [react()],
    define: { global: "globalThis" },
    resolve: {
      alias: [{ find: /^react-native$/, replacement: "react-native-web" }],
      extensions: [".web.tsx", ".web.ts", ".tsx", ".ts", ".jsx", ".js"],
    },
    server: {
      port: 5173,
      strictPort: true,
      watch: { ignored: ["**/android/**"] },
      proxy: {
        "/api": {
          target: "http://127.0.0.1:8787",
          changeOrigin: false,
          rewrite: (path) => path.replace(/^\/api/, ""),
          headers: token ? { Authorization: `Bearer ${token}` } : {},
        },
      },
    },
  };
});
