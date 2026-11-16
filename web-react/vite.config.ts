import path from 'path'
import { readFileSync } from 'node:fs'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const root = import.meta.dirname

const pkg = JSON.parse(
  readFileSync(path.resolve(root, './package.json'), 'utf-8'),
) as { version: string }

export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  server: {
    host: '0.0.0.0',
  },
  resolve: {
    alias: {
      '@': path.resolve(root, './src'),
      '@/hooks': path.resolve(root, './src/hooks'),
      '@/icons': path.resolve(root, './src/icons'),
      '@/components': path.resolve(root, './src/components'),
      '@/pages': path.resolve(root, './src/pages'),
      '@/context': path.resolve(root, './src/context'),
      '@/stores': path.resolve(root, './src/stores'),
      '@/services': path.resolve(root, './src/services'),
    },
  },
})
