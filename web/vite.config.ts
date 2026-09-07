import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

// 管理后台：开发时把 /api 与 /query 代理到本地 zerror-server
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: { '@': resolve(__dirname, 'src') },
  },
  build: {
    copyPublicDir: true,
    assetsDir: 'assets',
    assetsInlineLimit: 0,
    chunkSizeWarningLimit: 2000,
  },
  publicDir: 'public',
  server: {
    port: 1420,
    proxy: {
      '/api': 'http://127.0.0.1:3000',
      '/query': 'http://127.0.0.1:3000',
    },
  },
})
