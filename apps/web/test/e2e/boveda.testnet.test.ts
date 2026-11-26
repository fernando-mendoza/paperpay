/**
 * E2E en testnet de la bóveda (/boveda). SÍ mueve fondos de prueba: una cuenta desechable
 * (Friendbot) vacía la bóveda de ensayo. Nunca correrla contra la bóveda del reto.
 *   BOVEDA_E2E_CONTRACT=C… pnpm --filter @paperpay/web test:e2e
 *
 * Corre el código real del sitio (services/boveda). Solo Freighter está simulado: firma la llave
 * desechable.
 */
import { Keypair, Networks, TransactionBuilder } from '@stellar/stellar-sdk'
import { describe, expect, it, vi } from 'vitest'

const CONTRACT = process.env.BOVEDA_E2E_CONTRACT

const wallet = vi.hoisted(() => ({ keypair: null as import('@stellar/stellar-sdk').Keypair | null }))
vi.mock('@/services/wallet', () => ({
  signTxXdr: async (unsigned: string) => {
    const tx = TransactionBuilder.fromXDR(unsigned, Networks.TESTNET)
    tx.sign(wallet.keypair!)
    return tx.toXDR()
  },
}))

describe.skipIf(!CONTRACT)('bóveda en testnet', () => {
  it('cuenta nueva: Friendbot, código mal, "ship it" abre todo, otra vez está vacía', async () => {
    const { accountExists, activateWithFriendbot, claimVault, readVaultBalance, BovedaError } = await import('@/services/boveda')
    const contract = CONTRACT!
    wallet.keypair = Keypair.random()
    const me = wallet.keypair.publicKey()

    expect(await accountExists(me)).toBe(false)
    await expect(claimVault(me, 'Ship It', contract)).rejects.toMatchObject({ failure: 'NO_ACCOUNT' })
    await activateWithFriendbot(me)
    expect(await accountExists(me)).toBe(true)

    const before = await readVaultBalance(contract)
    expect(before).toBeGreaterThan(0n)

    const wrong = claimVault(me, 'Ship It now', contract)
    await expect(wrong).rejects.toBeInstanceOf(BovedaError)
    await expect(wrong).rejects.toMatchObject({ failure: 'WRONG_CODE' })
    expect(await readVaultBalance(contract)).toBe(before)

    const claimed = await claimVault(me, '  «ship it» ', contract)
    expect(claimed.amount).toBe(before)
    expect(claimed.hash).toMatch(/^[0-9a-f]{64}$/)
    expect(await readVaultBalance(contract)).toBe(0n)

    await expect(claimVault(me, 'Ship It', contract)).rejects.toMatchObject({ failure: 'EMPTY' })
    console.log(`claim ${claimed.hash} · ${claimed.amount} stroops a ${me}`)
  }, 180_000)
})
