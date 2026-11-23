// Pages Function: reenvía /api/* del dominio del frontend a la API de la VPS, igual que el
// rewrite de next.config.ts cuando hay servidor. Para el navegador es el mismo origen: sin CORS.
// API_ORIGIN (variable del proyecto de Pages) cambia el destino sin recompilar el frontend.
interface Env {
  API_ORIGIN?: string
}

const DEFAULT_API_ORIGIN = 'https://paperpay-api.blockchains.tf'

export const onRequest = async ({ request, env }: { request: Request; env: Env }): Promise<Response> => {
  const url = new URL(request.url)
  const target = new URL(url.pathname + url.search, env.API_ORIGIN || DEFAULT_API_ORIGIN)
  // new Request(target, request) copia método, headers (Authorization, PAYMENT-SIGNATURE) y
  // cuerpo. La respuesta vuelve tal cual: el 402 con PAYMENT-REQUIRED, o el 200 con
  // PAYMENT-RESPONSE.
  return fetch(new Request(target, request))
}
