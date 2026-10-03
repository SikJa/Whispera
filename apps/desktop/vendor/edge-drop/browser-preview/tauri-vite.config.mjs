import { defineConfig } from 'vite';
export default defineConfig({
  base: './', server: { host: '127.0.0.1', port: 5191, strictPort: true },
  plugins: [{ name: 'whispera-media', transform(code,id) {
    if (!id.includes('/src/components/') || !id.endsWith('.tsx')) return;
    return code.replace(/`edgelocal:\/\/\$\{item.data.imageId\}`/g,'item.data.preview')
      .replace(/`edgelocal:\/\/\$\{img.imageId\}`/g,'img.preview')
      .replace(/`edgelocal:\/\/file\/\$\{encodeURIComponent\((\w+).replace\(\/\\\\\/g, '\/'\)\)\}`/g,'((window as any).__previewFiles[$1] || "")')
      .replace(/e.preventDefault\(\)(\s+setInternalDragReq\(req\))/g,'if ((window as any).__TAURI_INTERNALS__) e.preventDefault(); else e.dataTransfer.setData("text/x-edge-preview", JSON.stringify(req));$1')
      .replaceAll('https://www.edgedrop.app/changelog','https://github.com/SikJa/Whispera/releases')
      .replaceAll('https://github.com/Deepender25/Edge-Drop','https://github.com/SikJa/Whispera')
      .replaceAll('Edge-Drop v','Whispera v');
  }}],
  build: { target: 'esnext', outDir: 'browser-dist', rollupOptions: { input: ['browser-preview.html', 'media.html'], output: {
    // Keep the upstream store/i18n/sound cycle in one chunk without eagerly mounting React.
    manualChunks(id) {
      if (id.includes('/node_modules/') || id.includes('/shared/') || (id.includes('/src/') && !id.endsWith('/src/main.tsx'))) return 'renderer';
    }
  } } }
});
