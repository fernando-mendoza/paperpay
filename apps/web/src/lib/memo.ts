// Memo de texto de una transacción de Stellar: hasta 28 bytes.
const MEMO_MAX_BYTES = 28

/**
 * Memo que pide la URL del artículo (`?memo=…`), p. ej. el código de un reto o una referencia.
 * Más de 28 bytes no cabe en una transacción de Stellar: se rechaza antes de abrir la wallet.
 */
export function memoFromUrl(search: string): string | undefined {
  const memo = new URLSearchParams(search).get('memo')?.trim()
  if (!memo) return undefined
  if (new TextEncoder().encode(memo).length > MEMO_MAX_BYTES) {
    throw new Error(`El memo pasa de ${MEMO_MAX_BYTES} bytes`)
  }
  return memo
}
