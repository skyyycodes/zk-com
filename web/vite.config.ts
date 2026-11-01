import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// `zsolv serve` serves the built app (web/dist) and the JSON API on :8080.
// During development, `npm run dev` proxies /api to that registry.
export default defineConfig({
  plugins: [react()],
  server: { proxy: { '/api': 'http://127.0.0.1:8080' } },
})
