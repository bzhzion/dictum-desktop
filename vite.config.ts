import { defineConfig } from 'vite';

// Port fixe et `strictPort` : Tauri pointe `devUrl` sur cette adresse exacte. Si Vite se
// rabattait sur un autre port parce que celui-ci est pris, la fenetre s'ouvrirait sur une page
// blanche sans dire pourquoi.
export default defineConfig({
  clearScreen: false,
  server: { port: 5183, strictPort: true },
  build: { target: 'es2022', emptyOutDir: true },
});
