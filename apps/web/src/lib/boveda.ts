/*
 * Bóveda del reto Ship It (contracts/boveda): guarda XLM de testnet y solo los suelta a quien
 * presenta el código. Lo que no depende de la red vive aquí, con pruebas (test/boveda.test.ts).
 */

/** La bóveda publicada (testnet). El script de lanzamiento del reto escribe este valor. */
export const BOVEDA_CONTRACT_ID = 'CA6UX6U2WMC6QTSAB53H7VA4NIXPXAFTYBV5BBOENXT6JL563WZYTPEI'

/** El código que espera el contrato (compara su SHA-256). */
export const BOVEDA_CODE = 'Ship It'

/** Fin de la vigilancia con relleno: 26-nov-2026 16:00 CDMX (22:00 UTC). */
export const BOVEDA_REFILL_UNTIL = Date.UTC(2026, 10, 26, 22, 0, 0)

export const BOVEDA_SOURCE_URL =
  'https://github.com/fernando-mendoza/paperpay/blob/main/contracts/boveda/src/lib.rs'

/**
 * Lo que escribe la persona, como lo recibe el contrato. El contrato compara exacto; aquí se
 * perdonan mayúsculas, comillas y espacios de más, para que «ship it» o "Ship  It" también abran.
 * Cualquier otro texto se manda tal cual (sin espacios en los extremos) y el contrato lo rechaza.
 */
export function normalizeCode(input: string): string {
  const bare = input
    .trim()
    .replace(/^["'«“‘`]+|["'»”’`]+$/g, '')
    .replace(/\s+/g, ' ')
    .trim()
  return bare.toLowerCase() === BOVEDA_CODE.toLowerCase() ? BOVEDA_CODE : bare
}

export type BovedaFailure = 'WRONG_CODE' | 'EMPTY' | 'OTHER'

/** Los errores del contrato (`Error(Contract, #n)` en la simulación) a algo que la pantalla explica. */
export function contractFailure(detail: string): BovedaFailure {
  if (/Error\(Contract, #2\)/.test(detail)) return 'WRONG_CODE'
  if (/Error\(Contract, #3\)/.test(detail)) return 'EMPTY'
  return 'OTHER'
}

/** Stroops (7 decimales) a XLM legible: 55550000000 → "5,555". Hasta 2 decimales. */
export function formatXlm(stroops: bigint): string {
  const whole = stroops / 10_000_000n
  const cents = Number((stroops % 10_000_000n) / 100_000n)
  const head = whole.toLocaleString('en-US')
  return cents ? `${head}.${String(cents).padStart(2, '0')}` : head
}

export function shortAddress(address: string): string {
  return `${address.slice(0, 4)}…${address.slice(-4)}`
}

export function explorerUrl(kind: 'contract' | 'tx' | 'account', id: string): string {
  return `https://stellar.expert/explorer/testnet/${kind}/${id}`
}
