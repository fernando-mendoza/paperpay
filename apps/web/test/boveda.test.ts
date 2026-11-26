import { describe, expect, it } from 'vitest'
import { BOVEDA_CODE, contractFailure, formatXlm, normalizeCode, shortAddress } from '@/lib/boveda'

describe('normalizeCode', () => {
  it('perdona mayúsculas, comillas y espacios de más', () => {
    for (const input of ['Ship It', 'ship it', 'SHIP IT', '  Ship   It ', '«Ship It»', '"ship it"', '“Ship It”', "'Ship It'"]) {
      expect(normalizeCode(input), input).toBe(BOVEDA_CODE)
    }
  })

  it('manda cualquier otro texto tal cual, para que el contrato lo rechace', () => {
    expect(normalizeCode('ShipIt')).toBe('ShipIt')
    expect(normalizeCode(' Ship It now ')).toBe('Ship It now')
    expect(normalizeCode('')).toBe('')
  })
})

describe('contractFailure', () => {
  it('traduce los errores de la bóveda que devuelve la simulación', () => {
    expect(contractFailure('HostError: Error(Contract, #2)\n...')).toBe('WRONG_CODE')
    expect(contractFailure('HostError: Error(Contract, #3)')).toBe('EMPTY')
    expect(contractFailure('HostError: Error(Contract, #1)')).toBe('OTHER')
    expect(contractFailure('Error(Auth, InvalidAction)')).toBe('OTHER')
  })
})

describe('formatXlm', () => {
  it('muestra XLM enteros con separador de miles y hasta 2 decimales', () => {
    expect(formatXlm(55_550_000_000n)).toBe('5,555')
    expect(formatXlm(0n)).toBe('0')
    expect(formatXlm(50_000_000n)).toBe('5')
    expect(formatXlm(12_345_678n)).toBe('1.23')
  })
})

it('shortAddress', () => {
  expect(shortAddress('CA74DQOQ7M6GYNCKMAZWP67OJ3TP6EEDO63FDPBSFYFECUCU3FFOKUNT')).toBe('CA74…KUNT')
})
