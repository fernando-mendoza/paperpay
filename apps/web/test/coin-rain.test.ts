import { describe, expect, it } from 'vitest'
import { COIN_COUNT, FLIGHT, TREASURY_COIN, destinationOf, flightOf, jitter, landed, releaseAt, usdc } from '../src/components/landing/coin-rain'

describe('lluvia de monedas del pago', () => {
  it('un pago de $0.50 son 50 centavos: 49 a la editorial y 1 a la tesorería (98/2)', () => {
    expect(COIN_COUNT).toBe(50)
    const all = landed(1)
    expect(all).toEqual({ publisher: 49, treasury: 1 })
    expect(usdc(all.publisher)).toBe('0.49')
    expect(usdc(all.treasury)).toBe('0.01')
    expect(destinationOf(TREASURY_COIN)).toBe('treasury')
  })

  it('antes del paso no hay monedas en ningún lado y todas llegan cuando se confirma (0.70)', () => {
    expect(landed(0)).toEqual({ publisher: 0, treasury: 0 })
    expect(landed(releaseAt(0) + FLIGHT - 0.001).publisher).toBe(0)
    expect(landed(releaseAt(COIN_COUNT - 1) + FLIGHT)).toEqual({ publisher: 49, treasury: 1 })
    expect(releaseAt(COIN_COUNT - 1) + FLIGHT).toBeCloseTo(0.7, 10)
  })

  it('las monedas llegan en orden y los contadores nunca bajan al avanzar', () => {
    let last = 0
    for (let p = 0; p <= 1; p += 0.01) {
      const { publisher, treasury } = landed(p)
      expect(publisher + treasury).toBeGreaterThanOrEqual(last)
      last = publisher + treasury
    }
    expect(flightOf(0, 0.5)).toBe(1)
    expect(flightOf(COIN_COUNT - 1, releaseAt(COIN_COUNT - 1))).toBe(0)
  })

  it('la lluvia es la misma en cada visita (sin Math.random)', () => {
    expect(jitter(3, 1)).toBe(jitter(3, 1))
    expect(jitter(3, 1)).not.toBe(jitter(4, 1))
    for (let i = 0; i < COIN_COUNT; i++) {
      expect(jitter(i, 2)).toBeGreaterThanOrEqual(0)
      expect(jitter(i, 2)).toBeLessThan(1)
    }
  })
})
