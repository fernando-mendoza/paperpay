/*
 * Enlaces de referido (`?ref=G…`). Quien comparte un artículo con el enlace de su wallet se lleva la
 * mitad de la comisión de PaperPay (`pay_ref` del contrato v0.4.0) cuando alguien paga entrando
 * por ese enlace. Quien paga paga lo mismo y la editorial recibe lo mismo (98%).
 *
 * - El enlace se guarda en este navegador 30 días; si llega otro, gana el último.
 * - El `ref` se quita de la URL al guardarlo: así, quien vuelve a compartir la página no reparte el
 *   enlace de otra persona.
 * - La API ignora el referido si es la misma wallet que paga o si esa cuenta no puede cobrar.
 */

const KEY = 'paperpay:ref'
/** Se emite en `window` al guardar un referido nuevo (la captura termina después del primer render). */
export const REFERRAL_EVENT = 'paperpay:referral'
export const REFERRAL_TTL_MS = 30 * 24 * 60 * 60 * 1000
const ACCOUNT_SHAPE = /^G[A-Z2-7]{55}$/

interface StoredReferral {
  address: string
  savedAt: number
}

function storage(): Storage | null {
  try {
    return window.localStorage
  } catch {
    return null
  }
}

/** El `ref` de una query string, o null. */
export function referralParam(search: string): string | null {
  const value = new URLSearchParams(search).get('ref')
  return value ? value.trim() : null
}

/** ¿Es una cuenta Stellar (G…) con checksum válido? */
export async function isStellarAccount(value: string): Promise<boolean> {
  if (!ACCOUNT_SHAPE.test(value)) return false
  const { StrKey } = await import('@stellar/stellar-sdk')
  return StrKey.isValidEd25519PublicKey(value)
}

/**
 * Lee `?ref=` de la URL actual: lo guarda si es una cuenta válida y lo quita de la URL.
 * Devuelve la cuenta guardada, o null si no había `ref` o no era válido.
 */
export async function captureReferral(now = Date.now()): Promise<string | null> {
  const ref = referralParam(window.location.search)
  if (ref === null) return null

  try {
    const url = new URL(window.location.href)
    url.searchParams.delete('ref')
    window.history.replaceState(window.history.state, '', url.pathname + url.search + url.hash)
  } catch {
    // Sin history (navegador muy viejo o pruebas): el ref se queda en la URL, no pasa nada.
  }

  if (!(await isStellarAccount(ref))) return null
  try {
    storage()?.setItem(KEY, JSON.stringify({ address: ref, savedAt: now } satisfies StoredReferral))
  } catch {
    return null // modo privado sin almacenamiento: no hay referido que recordar
  }
  window.dispatchEvent?.(new Event(REFERRAL_EVENT))
  return ref
}

/** El referido vigente para este pago: no vencido y distinto de quien paga. */
export function readReferral(payer?: string, now = Date.now()): string | undefined {
  let stored: Partial<StoredReferral>
  try {
    stored = JSON.parse(storage()?.getItem(KEY) ?? 'null') ?? {}
  } catch {
    return undefined
  }
  const { address, savedAt } = stored
  if (typeof address !== 'string' || !ACCOUNT_SHAPE.test(address)) return undefined
  if (typeof savedAt !== 'number' || now - savedAt > REFERRAL_TTL_MS || savedAt > now) return undefined
  if (address === payer) return undefined
  return address
}

/** El enlace para recomendar un artículo con tu wallet. */
export function referralLink(origin: string, paperId: string, address: string): string {
  return `${origin}/papers/${encodeURIComponent(paperId)}?ref=${address}`
}

/** `GABC…WXYZ` para mostrar una cuenta sin ocupar una línea entera. */
export function shortAccount(address: string): string {
  return `${address.slice(0, 4)}…${address.slice(-4)}`
}
