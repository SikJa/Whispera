import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  build: { rollupOptions: { input: { main: fileURLToPath(new URL('./index.html', import.meta.url)), overlay: fileURLToPath(new URL('./overlay.html', import.meta.url)) } } },
  plugins: [tailwindcss()],
  resolve: {
    dedupe: ["react", "react-dom", "motion", "clsx", "tailwind-merge"],
    alias: { "@": fileURLToPath(new URL("./vendor", import.meta.url)) },
  },
  server: { host: "127.0.0.1", port: 5190, strictPort: true },
});
