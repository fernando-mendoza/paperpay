'use client'

import { useCallback, useEffect, useState } from 'react'
import { COPY } from '@/lib/copy'
import { PaywallError } from '@/lib/errors'
import { canGift, giftLink, listGifts, sendGift, type SentGift } from '@/services/gift'
import { connectWallet } from '@/services/wallet'

type Step = 'idle' | 'connecting' | 'signing' | 'sending' | 'sent' | 'pending' | 'failed' | 'rejected'

function GiftIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden>
      <rect x="2.5" y="7" width="13" height="8.5" rx="1.5" stroke="currentColor" strokeWidth="1.4" />
      <path d="M1.75 4.75h14.5V7H1.75zM9 4.75V15.5" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
      <path
        d="M9 4.75C7.5 4.75 5.5 4 5.5 2.75S7.25 1.5 9 4.75zM9 4.75c1.5 0 3.5-.75 3.5-2S10.75 1.5 9 4.75z"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function CopyLink({ gift }: { gift: SentGift }) {
  const [copied, setCopied] = useState(false)
  const link = giftLink(gift)
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(link)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // El navegador bloqueó el portapapeles: el enlace sigue visible para copiarlo a mano.
    }
  }
  return (
    <div className="space-y-2">
      <input
        readOnly
        value={link}
        onFocus={(e) => e.currentTarget.select()}
        aria-label={COPY.gift.copy}
        className="w-full truncate rounded-lg border border-border bg-muted px-3 py-2 font-mono text-xs text-foreground"
      />
      <button
        type="button"
        onClick={copy}
        className={`min-h-[44px] w-full rounded-xl text-sm font-semibold transition-colors ${
          copied ? 'bg-success-bg text-success-strong' : 'bg-primary text-primary-foreground hover:bg-primary-hover'
        }`}
      >
        {copied ? COPY.gift.copied : COPY.gift.copy}
      </button>
    </div>
  )
}

/** "Regálalo": paga por el contrato (`gift`) y entrega un enlace que abre el artículo sin wallet. */
export function GiftPanel({ paperId }: { paperId: string }) {
  const [available, setAvailable] = useState(false)
  const [step, setStep] = useState<Step>('idle')
  const [current, setCurrent] = useState<SentGift | null>(null)
  const [history, setHistory] = useState<SentGift[]>([])

  const refresh = useCallback(() => setHistory(listGifts(paperId)), [paperId])

  useEffect(() => {
    let cancelled = false
    canGift(paperId).then((ok) => !cancelled && setAvailable(ok))
    refresh()
    return () => {
      cancelled = true
    }
  }, [paperId, refresh])

  const start = async () => {
    try {
      setStep('connecting')
      const wallet = await connectWallet()
      setStep('signing')
      const gift = await sendGift(paperId, wallet.address, () => setStep('sending'))
      setCurrent(gift)
      setStep('sent')
    } catch (err) {
      const code = err instanceof PaywallError ? err.code : 'PAYMENT_FAILED'
      setStep(code === 'PAYMENT_PENDING' ? 'pending' : code === 'USER_REJECTED' ? 'rejected' : 'failed')
    } finally {
      refresh()
    }
  }

  if (!available) return null
  const busy = step === 'connecting' || step === 'signing' || step === 'sending'
  const older = history.filter((g) => g.reader !== current?.reader)

  return (
    <section className="mt-4 overflow-hidden rounded-2xl border border-border bg-card shadow-[var(--shadow-card)]" aria-labelledby="gift-title">
      <div className="space-y-3 px-6 py-5">
        <div className="flex items-center gap-2 text-foreground">
          <GiftIcon />
          <h2 id="gift-title" className="text-sm font-semibold">
            {step === 'sent' ? COPY.gift.sentTitle : COPY.gift.title}
          </h2>
        </div>

        {step === 'sent' && current ? (
          <>
            <p className="text-xs text-muted-foreground">{COPY.gift.sentNote}</p>
            <CopyLink gift={current} />
            <button type="button" onClick={() => setStep('idle')} className="w-full text-center text-xs font-semibold text-primary hover:underline">
              {COPY.gift.another}
            </button>
          </>
        ) : (
          <>
            <p className="text-xs text-muted-foreground">
              {step === 'pending'
                ? COPY.gift.pending
                : step === 'failed'
                  ? COPY.gift.failed
                  : step === 'rejected'
                    ? COPY.gift.rejected
                    : COPY.gift.subtitle}
            </p>
            <button
              type="button"
              onClick={start}
              disabled={busy}
              aria-busy={busy}
              className="flex min-h-[48px] w-full items-center justify-center gap-2 rounded-xl border border-primary text-sm font-semibold text-primary transition-colors hover:bg-primary hover:text-primary-foreground disabled:cursor-wait disabled:opacity-70"
            >
              {step === 'connecting'
                ? COPY.gift.connecting
                : step === 'signing'
                  ? COPY.gift.signing
                  : step === 'sending'
                    ? COPY.gift.sending
                    : step === 'pending'
                      ? COPY.gift.retry
                      : COPY.gift.cta}
            </button>
          </>
        )}

        {older.length > 0 && (
          <details className="text-xs text-muted-foreground">
            <summary className="cursor-pointer font-medium">
              {COPY.gift.history} ({older.length})
            </summary>
            <ul className="mt-3 space-y-3">
              {older.map((gift) => (
                <li key={gift.reader}>
                  {gift.status === 'sent' ? <CopyLink gift={gift} /> : <span>{COPY.gift.historyPending}</span>}
                </li>
              ))}
            </ul>
          </details>
        )}
      </div>
    </section>
  )
}
