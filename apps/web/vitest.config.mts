import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

// Pruebas de la lógica del navegador (pago y panel) en Node: sin DOM, con fetch y window simulados.
export default defineConfig({
  resolve: {
    alias: [
      { find: /^@\//, replacement: fileURLToPath(new URL('./src/', import.meta.url)) },
      { find: '@paperpay/shared', replacement: fileURLToPath(new URL('../../packages/shared/src/index.ts', import.meta.url)) },
    ],
  },
  test: {
    environment: 'node',
    include: ['test/**/*.test.ts'],
    // Valores fijos: que un NEXT_PUBLIC_* del shell no cambie lo que se prueba.
    env: { NEXT_PUBLIC_API_URL: '', NEXT_PUBLIC_WALLET: 'freighter', NEXT_PUBLIC_DATA_SOURCE: 'api' },
  },
})
