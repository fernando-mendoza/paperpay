import { Keypair, StrKey, nativeToScVal } from '@stellar/stellar-sdk'
import { describe, expect, it } from 'vitest'
import { accountAddressParam, contractAddressParam, readScSymbol, readScVecLength } from '@/lib/soroban'
import { readsFromRecords, summarizeReads, type HorizonRecord } from '@/services/editorial'
import fixture from './fixtures/horizon-treasury-payments.json'
import payRefFixture from './fixtures/horizon-pay-ref.json'

// Contrato v0.2.0 y v0.1.0 en testnet (contracts/deployments/testnet.json).
const CONTRACT = 'CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5'
const OLD_CONTRACT = 'CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ'
const GIFT_PAYER = 'GDAORJSX3SR6HM5B4HGXJGJGBEZMNLCMWR5WI7EZDXX6YH2HR2EPQLDY'
const records = fixture.records as HorizonRecord[]
const { treasury } = fixture

const record = (hashPrefix: string) => records.find((r) => r.transaction_hash.startsWith(hashPrefix))!

describe('parámetros de Soroban en Horizon', () => {
  it('codifica la dirección del contrato igual que Horizon y que el SDK', () => {
    expect(contractAddressParam(CONTRACT)).toBe('AAAAEgAAAAEQyGLou3EgXRGEQAN3M7P17n/JvyVQMGB14qXEQB0Gow==')
    for (let i = 0; i < 20; i++) {
      const id = StrKey.encodeContract(Keypair.random().rawPublicKey())
      expect(contractAddressParam(id)).toBe(nativeToScVal(id, { type: 'address' }).toXDR('base64'))
    }
  })

  it('no acepta cuentas ni ids mal formados', () => {
    expect(contractAddressParam(treasury)).toBeNull()
    expect(contractAddressParam('CABC')).toBeNull()
  })

  it('codifica una cuenta G igual que el SDK (el referido de pay_ref)', () => {
    for (let i = 0; i < 20; i++) {
      const account = Keypair.random().publicKey()
      expect(accountAddressParam(account)).toBe(nativeToScVal(account, { type: 'address' }).toXDR('base64'))
    }
    expect(accountAddressParam(CONTRACT)).toBeNull()
    expect(accountAddressParam('GABC')).toBeNull()
  })

  it('lee la función llamada y el tamaño del paquete', () => {
    expect(readScSymbol(record('d505d693').parameters![1].value)).toBe('pay')
    expect(readScSymbol(record('8dac0f7e').parameters![1].value)).toBe('gift')
    expect(readScSymbol(record('551ae39b').parameters![1].value)).toBe('pay_batch')
    expect(readScSymbol(record('551ae39b').parameters![0].value)).toBeNull() // es una dirección
    expect(readScVecLength(record('551ae39b').parameters![3].value)).toBe(6)
    expect(readScVecLength(record('d505d693').parameters![1].value)).toBe(0)
    expect(readScSymbol(undefined)).toBeNull()
  })
})

describe('lecturas de la tesorería', () => {
  it('sin contrato anunciado cuenta solo las transferencias directas (reparto estimado)', () => {
    const stats = summarizeReads(readsFromRecords(records, { treasury, splitterContractIds: [] }), treasury, false)
    expect(stats.reads).toBe(25)
    expect(stats.contractReads).toBe(0)
    expect(stats.totalUsdc).toBe(12.5)
    expect(stats.publisherUsdc).toBe(12.25)
    expect(stats.feeUsdc).toBe(0.25)
  })

  it('suma las lecturas del contrato con el reparto real de cada transacción', () => {
    const entries = readsFromRecords(records, { treasury, splitterContractIds: [CONTRACT] })
    const contract = entries.filter((e) => e.kind === 'contract')
    expect(contract.map((e) => [e.hash.slice(0, 8), e.reads, e.amount, e.publisherStroops, e.feeStroops])).toEqual([
      ['d505d693', 1, 0.5, 4_900_000n, 100_000n],
      ['b9b9a5ad', 1, 0.5, 4_900_000n, 100_000n],
      ['8dac0f7e', 1, 0.5, 4_900_000n, 100_000n],
      ['551ae39b', 6, 3, 29_400_000n, 600_000n],
    ])

    const stats = summarizeReads(entries, treasury, false)
    expect(stats.reads).toBe(34) // 25 directas + 1 + 1 + 1 + 6
    expect(stats.contractReads).toBe(9)
    expect(stats.totalUsdc).toBe(17)
    expect(stats.publisherUsdc).toBe(16.66) // 12.25 estimado + 4.41 real
    expect(stats.feeUsdc).toBe(0.34) // 0.25 estimado + 0.09 real
  })

  it('en gift, la cuenta que pagó es la que regala', () => {
    const gift = readsFromRecords(records, { treasury, splitterContractIds: [CONTRACT] }).find((e) => e.hash.startsWith('8dac0f7e'))
    expect(gift?.from).toBe(GIFT_PAYER)
  })

  it('no cuenta llamadas de otro contrato', () => {
    const current = readsFromRecords(records, { treasury, splitterContractIds: [CONTRACT] })
    expect(current.some((e) => e.hash.startsWith('cdda69b1'))).toBe(false)
    const old = readsFromRecords(records, { treasury, splitterContractIds: [OLD_CONTRACT] }).filter((e) => e.kind === 'contract')
    expect(old.map((e) => e.hash.slice(0, 8))).toEqual(['cdda69b1'])
  })

  it('no cuenta funciones que no cobran ni transacciones fallidas', () => {
    const pay = record('d505d693')
    const split = { ...pay, parameters: [pay.parameters![0], { type: 'Sym', value: nativeToScVal('split', { type: 'symbol' }).toXDR('base64') }] }
    const failed = { ...pay, transaction_successful: false }
    expect(readsFromRecords([split, failed], { treasury, splitterContractIds: [CONTRACT] })).toEqual([])
  })

  it('agrupa las lecturas por día (los 14 días previos a hoy)', () => {
    const entries = readsFromRecords(records, { treasury, splitterContractIds: [CONTRACT] })
    const stats = summarizeReads(entries, treasury, false, new Date('2026-11-24T12:00:00Z'))
    expect(stats.perDay).toHaveLength(14)
    expect(stats.perDay.at(-1)?.count).toBe(9) // los 4 pagos por contrato del 24-nov (UTC)
    expect(stats.perDay.reduce((sum, d) => sum + d.count, 0)).toBe(9) // las directas son de octubre
    expect(stats.recent).toHaveLength(8)
    expect(stats.recent[0]).not.toHaveProperty('publisherStroops')
  })
})

describe('pay_ref y contratos anteriores', () => {
  // v0.4.0 en testnet; el pay_ref real con su registro de Horizon (contracts/deployments/testnet.json).
  const V040 = 'CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA'
  const payRef = payRefFixture.records as HorizonRecord[]

  it('pay_ref es una lectura; lo que cobra quien recomendó cuenta como comisión (98/2 intacto)', () => {
    const [entry] = readsFromRecords(payRef, { treasury, splitterContractIds: [V040] })
    expect(readScSymbol(payRef[0].parameters![1].value)).toBe('pay_ref')
    expect(entry).toMatchObject({ kind: 'contract', reads: 1, amount: 0.5, publisherStroops: 4_900_000n, feeStroops: 100_000n })
  })

  it('suma las lecturas del contrato activo y de los anteriores', () => {
    const all = [...payRef, ...records]
    const onlyNew = readsFromRecords(all, { treasury, splitterContractIds: [V040] }).filter((e) => e.kind === 'contract')
    const both = readsFromRecords(all, { treasury, splitterContractIds: [V040, CONTRACT] }).filter((e) => e.kind === 'contract')
    expect(onlyNew.reduce((n, e) => n + e.reads, 0)).toBe(1)
    expect(both.reduce((n, e) => n + e.reads, 0)).toBe(10) // 1 pay_ref + las 9 de v0.2.0
  })
})
