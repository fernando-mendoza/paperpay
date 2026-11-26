// La lluvia de monedas del paso "Stellar lo liquida": un pago de $0.50 son 50 monedas de un centavo.
// 49 caen en la editorial (98%) y 1 en la tesorería de PaperPay (2%), como reparte el contrato.
// Todo es función del progreso del paso (0→1): el canvas y los contadores leen lo mismo, así que
// los números nunca se adelantan a las monedas, y el scroll hacia atrás lo deshace.

/** Monedas de un centavo en un pago de $0.50. */
export const COIN_COUNT = 50
/** La moneda que va a la tesorería: la del medio del chorro, para que se vea sola. */
export const TREASURY_COIN = 24
/** Progreso en que sale la primera moneda y en que sale la última. */
const FIRST_RELEASE = 0.12
const LAST_RELEASE = 0.45
/** Lo que tarda una moneda en caer, en progreso del paso. Las últimas llegan a 0.70, cuando se confirma. */
export const FLIGHT = 0.25

export type Destination = 'publisher' | 'treasury'

export function destinationOf(index: number): Destination {
  return index === TREASURY_COIN ? 'treasury' : 'publisher'
}

/** Cuándo sale la moneda `index` (progreso del paso). */
export function releaseAt(index: number): number {
  return FIRST_RELEASE + ((LAST_RELEASE - FIRST_RELEASE) * index) / (COIN_COUNT - 1)
}

/** 0 antes de salir, 0→1 mientras cae y 1 cuando ya llegó. */
export function flightOf(index: number, progress: number): number {
  const f = (progress - releaseAt(index)) / FLIGHT
  // El margen absorbe el redondeo de coma flotante: en el progreso exacto de llegada, ya llegó.
  return f >= 1 - 1e-9 ? 1 : Math.max(0, f)
}

/** Monedas que ya llegaron a cada destino. */
export function landed(progress: number): { publisher: number; treasury: number } {
  let publisher = 0
  let treasury = 0
  for (let i = 0; i < COIN_COUNT; i++) {
    if (flightOf(i, progress) < 1) continue
    if (destinationOf(i) === 'treasury') treasury++
    else publisher++
  }
  return { publisher, treasury }
}

/** Centavos a USDC con dos decimales: 49 → "0.49". */
export function usdc(cents: number): string {
  return (cents / 100).toFixed(2)
}

/** Número pseudoaleatorio estable por moneda (0→1): la lluvia es igual en cada visita y en cada captura. */
export function jitter(index: number, salt: number): number {
  const x = Math.sin(index * 127.1 + salt * 311.7) * 43758.5453
  return x - Math.floor(x)
}
