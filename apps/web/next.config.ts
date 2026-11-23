import type { NextConfig } from 'next'

// El navegador llama a /api/* en el mismo dominio del frontend y ese dominio lo reenvía al backend.
// Así CORS no depende de qué dominio tenga cada deploy (producción, previews de PR o localhost).
//   - Con servidor (next start, Vercel): lo reenvía el rewrite de abajo.
//   - Export estático (STATIC_EXPORT=1, Cloudflare Pages): Next no tiene rewrites; lo reenvía la
//     Pages Function de deploy/cloudflare-pages/functions/api/[[path]].ts.
const apiProxyTarget = (process.env.API_PROXY_TARGET ?? 'https://paperpay-api.blockchains.tf').replace(/\/$/, '')
const staticExport = process.env.STATIC_EXPORT === '1'

const nextConfig: NextConfig = {
  // En dev, React Strict Mode monta el provider de Pollar dos veces y crea dos
  // PollarClient para la misma API key (avisa que la detección de reuso puede cerrar
  // la sesión). Solo afecta a dev; el build de producción no re-monta. Se desactiva
  // para tener una sola instancia del SDK durante las pruebas de Pollar (v2).
  reactStrictMode: false,
  transpilePackages: ['@paperpay/shared', '@pollar/react', '@pollar/core'],
  ...(staticExport
    ? { output: 'export' as const }
    : {
        async rewrites() {
          return [{ source: '/api/:path*', destination: `${apiProxyTarget}/api/:path*` }]
        },
      }),
}

export default nextConfig
