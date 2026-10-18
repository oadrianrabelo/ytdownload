import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// No Android, o celular acessa o servidor de desenvolvimento pela rede.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 5174 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/plugins/**"] },
  },
  build: { target: "es2021", outDir: "dist", emptyOutDir: true },
});
