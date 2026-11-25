'use client'

import { useState } from 'react'
import { COPY } from '@/lib/copy'
import { PaywallError } from '@/lib/errors'
import { referralLink } from '@/lib/referral'
import { connectWallet } from '@/services/wallet'

type Step = 'idle' | 'connecting' | 'ready' | 'failed' | 'rejected'

function ShareIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden>
      <circle cx="13.5" cy="4" r="2.25" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="4.5" cy="9" r="2.25" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="13.5" cy="14" r="2.25" stroke="currentColor" strokeWidth="1.4" />
      <path d="M6.5 7.9l5-2.8M6.5 10.1l5 2.8" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  )
}

/**
 * "Recomiéndalo y gana": arma el enlace del artículo con la wallet de quien recomienda. Quien pague
 * entrando por él firma `pay_ref` y esa wallet recibe la mitad de la comisión. Solo se muestra si
 * la API paga referidos (`useReferral`).
 */
export function ReferPanel({ paperId }: { paperId: string }) {
  const [step, setStep] = useState<Step>('idle')
  const [link, setLink] = useState('')
  const [copied, setCopied] = useState(false)

  const start = async () => {
    try {
      setStep('connecting')
      const wallet = await connectWallet()
      setLink(referralLink(window.location.origin, paperId, wallet.address))
      setStep('ready')
    } catch (err) {
      setStep(err instanceof PaywallError && err.code === 'USER_REJECTED' ? 'rejected' : 'failed')
    }
  }

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
    <section className="mt-4 overflow-hidden rounded-2xl border border-border bg-card shadow-[var(--shadow-card)]" aria-labelledby="refer-title">
      <div className="space-y-3 px-6 py-5">
        <div className="flex items-center gap-2 text-foreground">
          <ShareIcon />
          <h2 id="refer-title" className="text-sm font-semibold">
            {step === 'ready' ? COPY.refer.ready : COPY.refer.title}
          </h2>
        </div>

        {step === 'ready' ? (
          <div className="space-y-2">
            <input
              readOnly
              value={link}
              onFocus={(e) => e.currentTarget.select()}
              aria-label={COPY.refer.copy}
              className="w-full truncate rounded-lg border border-border bg-muted px-3 py-2 font-mono text-xs text-foreground"
            />
            <button
              type="button"
              onClick={copy}
              className={`min-h-[44px] w-full rounded-xl text-sm font-semibold transition-colors ${
                copied ? 'bg-success-bg text-success-strong' : 'bg-primary text-primary-foreground hover:bg-primary-hover'
              }`}
            >
              {copied ? COPY.refer.copied : COPY.refer.copy}
            </button>
            <p className="text-xs text-muted-foreground">{COPY.refer.note}</p>
          </div>
        ) : (
          <>
            <p className="text-xs text-muted-foreground">
              {step === 'failed' ? COPY.refer.failed : step === 'rejected' ? COPY.refer.rejected : COPY.refer.subtitle}
            </p>
            <button
              type="button"
              onClick={start}
              disabled={step === 'connecting'}
              aria-busy={step === 'connecting'}
              className="flex min-h-[48px] w-full items-center justify-center gap-2 rounded-xl border border-primary text-sm font-semibold text-primary transition-colors hover:bg-primary hover:text-primary-foreground disabled:cursor-wait disabled:opacity-70"
            >
              {step === 'connecting' ? COPY.refer.connecting : COPY.refer.cta}
            </button>
          </>
        )}
      </div>
    </section>
  )
}
