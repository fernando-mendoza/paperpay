'use client'

import { useCallback, useEffect, useState } from 'react'
import {
  BOVEDA_CONTRACT_ID,
  BOVEDA_REFILL_UNTIL,
  BOVEDA_SOURCE_URL,
  explorerUrl,
  formatXlm,
  shortAddress,
} from '@/lib/boveda'
import { COPY } from '@/lib/copy'
import { PaywallError } from '@/lib/errors'
import { BovedaError, accountExists, activateWithFriendbot, claimVault, readVaultBalance } from '@/services/boveda'
import { connectFreighter } from '@/services/wallet'

const T = COPY.boveda

type Phase = 'idle' | 'connecting' | 'activating' | 'claiming'
type Result = { hash: string; amount: bigint }

function errorMessage(err: unknown): string {
  if (err instanceof PaywallError) {
    if (err.code === 'NO_WALLET') return T.noWallet
    if (err.code === 'WRONG_NETWORK') return T.wrongNetwork
    if (err.code === 'USER_REJECTED') return T.rejected
  }
  if (err instanceof BovedaError) {
    if (err.failure === 'WRONG_CODE') return T.wrongCode
    if (err.failure === 'EMPTY') return Date.now() < BOVEDA_REFILL_UNTIL ? `${T.empty} ${T.refillNote}` : T.empty
    if (err.failure === 'NO_ACCOUNT') return T.noAccount
    if (err.failure === 'REJECTED') return T.rejected
    if (err.failure === 'TIMEOUT') return T.timeout
  }
  return err instanceof Error && err.message ? `${T.failed} ${err.message}` : T.failed
}

/** `/boveda`: conecta Freighter (testnet), escribe el código y abre la bóveda del reto Ship It. */
export function BovedaView() {
  const [balance, setBalance] = useState<bigint | null | 'error'>(null)
  const [address, setAddress] = useState<string | null>(null)
  const [exists, setExists] = useState(true)
  const [code, setCode] = useState('')
  const [phase, setPhase] = useState<Phase>('idle')
  const [step, setStep] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<Result | null>(null)

  const refresh = useCallback(() => {
    readVaultBalance()
      .then(setBalance)
      .catch(() => setBalance((prev) => (prev === null ? 'error' : prev)))
  }, [])

  useEffect(() => {
    refresh()
    const id = setInterval(refresh, 10_000)
    return () => clearInterval(id)
  }, [refresh])

  const connect = async () => {
    setPhase('connecting')
    setError(null)
    try {
      const wallet = await connectFreighter()
      // Primero se sabe si la cuenta existe; así no aparece el campo del código antes de tiempo.
      const found = await accountExists(wallet.address).catch(() => true)
      setExists(found)
      setAddress(wallet.address)
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setPhase('idle')
    }
  }

  const activate = async () => {
    if (!address) return
    setPhase('activating')
    setError(null)
    try {
      await activateWithFriendbot(address)
      setExists(true)
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      setPhase('idle')
    }
  }

  const claim = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!address || !code.trim()) return
    setPhase('claiming')
    setError(null)
    setResult(null)
    setStep(T.simulating)
    // La simulación tarda ~1 s; después Freighter abre su ventana.
    const signingHint = setTimeout(() => setStep(T.signing), 1200)
    try {
      const claimed = await claimVault(address, code)
      setResult(claimed)
      setCode('')
    } catch (err) {
      setError(errorMessage(err))
    } finally {
      clearTimeout(signingHint)
      setStep(null)
      setPhase('idle')
      refresh()
    }
  }

  const busy = phase !== 'idle'

  return (
    <main className="mx-auto max-w-xl px-4 py-12 sm:px-6 sm:py-20">
      <div className="overflow-hidden rounded-2xl border border-border bg-card shadow-[var(--shadow-elevated)]">
        <div className="space-y-6 px-6 py-8 sm:px-10">
          <header className="space-y-3 text-center">
            <p className="text-xs font-semibold tracking-wide text-primary uppercase">{T.eyebrow}</p>
            <h1 className="font-display text-3xl leading-tight font-semibold text-foreground">{T.title}</h1>
            <p className="text-sm leading-relaxed text-muted-foreground">{T.lead}</p>
          </header>

          <section className="rounded-xl bg-muted px-5 py-4 text-center" aria-live="polite">
            <p className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">{T.balanceLabel}</p>
            <p className="mt-1 font-mono text-3xl font-semibold text-foreground" data-testid="boveda-balance">
              {balance === null ? T.balanceLoading : balance === 'error' ? T.balanceError : `${formatXlm(balance)} XLM`}
            </p>
            <a
              href={explorerUrl('contract', BOVEDA_CONTRACT_ID)}
              target="_blank"
              rel="noreferrer"
              className="mt-2 inline-block text-xs font-semibold text-primary hover:underline"
            >
              {T.contract} · {shortAddress(BOVEDA_CONTRACT_ID)}
            </a>
          </section>

          {!address ? (
            <button
              type="button"
              onClick={connect}
              disabled={busy}
              aria-busy={phase === 'connecting'}
              className="min-h-[52px] w-full rounded-xl bg-primary text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary-hover disabled:cursor-wait disabled:opacity-70"
            >
              {phase === 'connecting' ? T.connecting : T.connect}
            </button>
          ) : (
            <div className="space-y-4">
              <p className="text-center text-xs text-muted-foreground">
                {T.connected}:{' '}
                <a href={explorerUrl('account', address)} target="_blank" rel="noreferrer" className="font-mono text-foreground hover:underline">
                  {shortAddress(address)}
                </a>
              </p>

              {!exists ? (
                <div className="space-y-3 text-center">
                  <p className="text-sm text-foreground">{T.noAccount}</p>
                  <button
                    type="button"
                    onClick={activate}
                    disabled={busy}
                    aria-busy={phase === 'activating'}
                    className="min-h-[48px] w-full rounded-xl border border-primary text-sm font-semibold text-primary transition-colors hover:bg-muted disabled:cursor-wait disabled:opacity-70"
                  >
                    {phase === 'activating' ? T.activating : T.activate}
                  </button>
                </div>
              ) : (
                <form onSubmit={claim} className="space-y-3">
                  <label htmlFor="boveda-code" className="block text-sm font-semibold text-foreground">
                    {T.codeLabel}
                  </label>
                  <input
                    id="boveda-code"
                    type="text"
                    autoComplete="off"
                    autoCapitalize="off"
                    spellCheck={false}
                    value={code}
                    onChange={(e) => setCode(e.target.value)}
                    placeholder={T.codePlaceholder}
                    disabled={busy}
                    className="min-h-[48px] w-full rounded-xl border border-border bg-background px-4 text-base text-foreground outline-none focus:border-primary"
                  />
                  <button
                    type="submit"
                    disabled={busy || !code.trim()}
                    aria-busy={phase === 'claiming'}
                    className="min-h-[52px] w-full rounded-xl bg-primary text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary-hover disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {phase === 'claiming' && step ? step : T.claim}
                  </button>
                </form>
              )}
            </div>
          )}

          {error && (
            <p role="alert" className="rounded-lg bg-error-bg px-3 py-2 text-sm text-[#B91C1C]" data-testid="boveda-error">
              {error}
            </p>
          )}

          {result && (
            <div className="space-y-1 rounded-xl border border-success bg-success-bg px-4 py-4 text-center" role="status" data-testid="boveda-success">
              <p className="text-sm font-semibold text-foreground">{T.successTitle}</p>
              <p className="font-mono text-2xl font-semibold text-foreground">{formatXlm(result.amount)} XLM</p>
              <p className="text-xs text-muted-foreground">{T.successBody}</p>
              <a href={explorerUrl('tx', result.hash)} target="_blank" rel="noreferrer" className="inline-block text-xs font-semibold text-primary hover:underline">
                {T.tx} · {result.hash.slice(0, 8)}…
              </a>
            </div>
          )}

          <section className="space-y-2 border-t border-border pt-5">
            <h2 className="text-sm font-semibold text-foreground">{T.howTitle}</h2>
            <ul className="list-disc space-y-1 pl-5 text-xs leading-relaxed text-muted-foreground">
              {T.how.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
            <a href={BOVEDA_SOURCE_URL} target="_blank" rel="noreferrer" className="inline-block text-xs font-semibold text-primary hover:underline">
              {T.source}
            </a>
          </section>
        </div>
      </div>
    </main>
  )
}
