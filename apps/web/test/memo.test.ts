import { describe, expect, it } from 'vitest'
import { memoFromUrl } from '@/lib/memo'

describe('memoFromUrl', () => {
  it('lee ?memo= y recorta espacios; sin memo o vacío → undefined', () => {
    expect(memoFromUrl('?wallet=freighter&memo=%20SHIP-ABC123%20')).toBe('SHIP-ABC123')
    expect(memoFromUrl('?wallet=freighter')).toBeUndefined()
    expect(memoFromUrl('?memo=')).toBeUndefined()
  })

  it('acepta hasta 28 bytes (los acentos cuentan doble) y rechaza más', () => {
    expect(memoFromUrl(`?memo=${'x'.repeat(28)}`)).toBe('x'.repeat(28))
    expect(() => memoFromUrl(`?memo=${'x'.repeat(29)}`)).toThrow(/28 bytes/)
    expect(() => memoFromUrl(`?memo=${encodeURIComponent('é'.repeat(15))}`)).toThrow(/28 bytes/)
  })
})
