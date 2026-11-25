import { Keypair } from '@stellar/stellar-sdk'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { captureReferral, readReferral, REFERRAL_TTL_MS, referralLink, referralParam, shortAccount } from '@/lib/referral'

const friend = Keypair.random().publicKey()
const reader = Keypair.random().publicKey()

function memoryStorage(): Storage {
  const data = new Map<string, string>()
  return {
    getItem: (k) => data.get(k) ?? null,
    setItem: (k, v) => void data.set(k, String(v)),
    removeItem: (k) => void data.delete(k),
    clear: () => data.clear(),
    key: (i) => [...data.keys()][i] ?? null,
    get length() {
      return data.size
    },
  }
}

let replaced: string[] = []

function visit(url: string) {
  const parsed = new URL(url)
  replaced = []
  vi.stubGlobal('window', {
    ...globalThis.window,
    location: { href: parsed.href, search: parsed.search },
    history: { state: null, replaceState: (_: unknown, __: string, next: string) => replaced.push(next) },
  })
}

beforeEach(() => {
  vi.stubGlobal('window', { localStorage: memoryStorage() })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('enlaces de referido', () => {
  it('guarda ?ref= de una cuenta válida y lo quita de la URL (conserva lo demás)', async () => {
    visit(`https://paperpay.blockchains.tf/papers/crispr?wallet=freighter&ref=${friend}#resumen`)
    expect(await captureReferral()).toBe(friend)
    expect(replaced).toEqual(['/papers/crispr?wallet=freighter#resumen'])
    expect(readReferral(reader)).toBe(friend)
  })

  it('ignora un ref que no es cuenta Stellar (forma o checksum), pero igual lo quita de la URL', async () => {
    const badChecksum = friend.slice(0, -1) + (friend.endsWith('A') ? 'B' : 'A')
    for (const bad of ['hola', 'CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5', badChecksum]) {
      visit(`https://paperpay.blockchains.tf/lee?ref=${bad}`)
      expect(await captureReferral()).toBeNull()
      expect(replaced).toEqual(['/lee'])
    }
    expect(readReferral(reader)).toBeUndefined()
  })

  it('sin ?ref= no toca la URL ni lo guardado', async () => {
    visit(`https://paperpay.blockchains.tf/lee?ref=${friend}`)
    await captureReferral()
    visit('https://paperpay.blockchains.tf/top')
    expect(await captureReferral()).toBeNull()
    expect(replaced).toEqual([])
    expect(readReferral(reader)).toBe(friend)
  })

  it('gana el último enlace', async () => {
    const other = Keypair.random().publicKey()
    visit(`https://paperpay.blockchains.tf/lee?ref=${friend}`)
    await captureReferral()
    visit(`https://paperpay.blockchains.tf/top?ref=${other}`)
    await captureReferral()
    expect(readReferral(reader)).toBe(other)
  })

  it('vence a los 30 días y no aplica a la misma wallet que paga', async () => {
    const t0 = 1_800_000_000_000
    visit(`https://paperpay.blockchains.tf/lee?ref=${friend}`)
    await captureReferral(t0)
    expect(readReferral(reader, t0 + REFERRAL_TTL_MS)).toBe(friend)
    expect(readReferral(reader, t0 + REFERRAL_TTL_MS + 1)).toBeUndefined()
    expect(readReferral(friend, t0 + 1)).toBeUndefined()
  })

  it('lo guardado con otra forma se ignora', () => {
    window.localStorage.setItem('paperpay:ref', '{roto')
    expect(readReferral(reader)).toBeUndefined()
    window.localStorage.setItem('paperpay:ref', JSON.stringify({ address: 'GABC', savedAt: Date.now() }))
    expect(readReferral(reader)).toBeUndefined()
  })

  it('arma el enlace del artículo y acorta la cuenta', () => {
    expect(referralLink('https://paperpay.blockchains.tf', 'crispr nano', friend)).toBe(`https://paperpay.blockchains.tf/papers/crispr%20nano?ref=${friend}`)
    expect(referralParam(`?ref=${friend}`)).toBe(friend)
    expect(shortAccount(friend)).toBe(`${friend.slice(0, 4)}…${friend.slice(-4)}`)
  })
})
