import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  plugins: [tailwindcss()],
  resolve: {
    dedupe: ["react", "react-dom", "motion", "clsx", "tailwind-merge"],
    alias: { "@": fileURLToPath(new URL("./vendor", import.meta.url)) },
  },
  server: { host: "127.0.0.1", port: 5190, strictPort: true },
});
