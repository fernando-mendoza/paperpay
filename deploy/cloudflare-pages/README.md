# Frontend en Cloudflare Pages (export estático)

El frontend corre sin servidor de Next en Cloudflare Pages. Todo es HTML y JS estático salvo `/api/*`, que lo atiende una Pages Function y lo reenvía a la API de la VPS:

```
navegador ─► paperpay.blockchains.tf (Cloudflare Pages)
             ├─ /, /papers/<id>, /editorial, /_next/* ─► archivos estáticos del export de Next
             └─ /api/*  ─► functions/api/[[path]].ts ─► https://paperpay-api.blockchains.tf/api/*
```

Para el navegador todo es el mismo origen, igual que con el rewrite de `next.config.ts` en Vercel: no hace falta CORS y los previews de Pages funcionan sin tocar la API.

| Archivo | Qué es |
|---|---|
| `functions/api/[[path]].ts` | Proxy de `/api/*`: copia método, headers y cuerpo, y devuelve la respuesta tal cual (402 incluido). La variable `API_ORIGIN` del proyecto cambia el destino sin recompilar. |
| `_headers` | Cabeceras de seguridad para los estáticos y caché larga para `/_next/static/*`. |
| `../../scripts/deploy-web-cf-pages.sh` | Typecheck, export (`STATIC_EXPORT=1`), copia `_headers` y publica con wrangler. |

## Qué cambia con `STATIC_EXPORT=1`

- `next.config.ts` pasa a `output: 'export'` y quita el rewrite (el export no lo soporta).
- `/papers/[id]` se pre-renderiza con `generateStaticParams` a partir del catálogo del frontend (`src/mocks/papers.json`, los mismos 52 ids que la API). **Un artículo nuevo en la API pide recompilar el frontend**; sin eso, su página da 404.
- Las variables `NEXT_PUBLIC_*` se hornean al compilar. El script fija `NEXT_PUBLIC_DATA_SOURCE=api`, `NEXT_PUBLIC_API_URL` vacío (llamadas relativas) y por defecto `NEXT_PUBLIC_WALLET=freighter`. Pollar (v2) solo se activa si se pasa `NEXT_PUBLIC_POLLAR_PUBLISHABLE_KEY`.

## Primera vez

1. Crear el proyecto con la rama de producción `main`:
   ```bash
   pnpm dlx wrangler@4.158.0 pages project create paperpay --production-branch main
   ```
2. Publicar (abajo).
3. Dominio: agregarlo en el proyecto (*Custom domains*) y crear a mano un `CNAME` proxied del subdominio hacia `<proyecto>.pages.dev`. Agregarlo por API no crea el registro DNS y el dominio se queda en `initializing`.

## Publicar

```bash
CLOUDFLARE_ACCOUNT_ID=<id> CLOUDFLARE_API_TOKEN=<token> bash scripts/deploy-web-cf-pages.sh
```

- Fija siempre `CLOUDFLARE_ACCOUNT_ID`: si el token ve varias cuentas, wrangler puede elegir otra y fallar con `Authentication error [code: 10000]`.
- `CF_PAGES_BRANCH=main` (por defecto) publica a producción. Cualquier otra rama crea un preview en `<rama>.<proyecto>.pages.dev` y no toca el dominio.
- Se niega a correr si `apps/web`, `packages/shared`, `deploy/cloudflare-pages` o el lockfile tienen cambios sin commitear.

## Rollback

En el panel del proyecto, *Deployments* → el deployment anterior → *Rollback to this deployment*. Es instantáneo y no recompila.

## Límites conocidos

- Las invocaciones de la Pages Function cuentan contra el límite diario de Workers + Functions de la cuenta (100k/día en el plan Free, compartido con los demás proyectos de esa cuenta). Los estáticos no cuentan.
- Sin servidor de Next no hay rutas dinámicas fuera del catálogo compilado ni render en servidor.
