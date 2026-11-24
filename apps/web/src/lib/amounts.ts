// Montos de Stellar: 7 decimales. Se suman en stroops (enteros) para no arrastrar error de
// punto flotante; se convierten a USDC solo al mostrar.

export const STROOPS_PER_USDC = 10_000_000

/** "0.4900000" (formato de Horizon) → 4900000n. */
export function toStroops(amount: string): bigint {
  const [whole, fraction = ''] = amount.trim().split('.')
  return BigInt(whole || '0') * BigInt(STROOPS_PER_USDC) + BigInt(fraction.padEnd(7, '0').slice(0, 7))
}

export function stroopsToUsdc(stroops: bigint | number): number {
  return Number(stroops) / STROOPS_PER_USDC
}
