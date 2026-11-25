'use client'

import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { useEffect, useState } from 'react'
import type { PaymentReceiptResponse } from '@paperpay/shared'
import { COPY } from '@/lib/copy'
import { claimGift, fetchGiftReceipt, parseGiftFragment } from '@/services/gift'

type View =
  | { status: 'loading' }
  | { status: 'invalid' }
  | { status: 'notYet' }
  | { status: 'ready'; receipt: PaymentReceiptResponse | null; opening: boolean; error: string | null }

/** `/regalo#<txHash>.<secreto>`: abre un artículo que otra persona pagó. */
export function GiftClaimView() {
  const router = useRouter()
  const [view, setView] = useState<View>({ status: 'loading' })
  const [link, setLink] = useState<{ txHash: string; secret: string } | null>(null)

  useEffect(() => {
    const parsed = parseGiftFragment(window.location.hash)
    if (!parsed) {
      setView({ status: 'invalid' })
      return
    }
    setLink(parsed)
    fetchGiftReceipt(parsed.txHash)
      .then((receipt) => setView(receipt ? { status: 'ready', receipt, opening: false, error: null } : { status: 'notYet' }))
      // Sin recibo legible (API caída): se puede intentar abrir igual.
      .catch(() => setView({ status: 'ready', receipt: null, opening: false, error: null }))
  }, [])

  const open = async () => {
    if (!link || view.status !== 'ready') return
    setView({ ...view, opening: true, error: null })
    try {
      const opened = await claimGift(link.txHash, link.secret)
      router.push(`/papers/${encodeURIComponent(opened.paperId)}`)
    } catch (err) {
      setView({ ...view, opening: false, error: err instanceof Error && err.message ? err.message : COPY.giftClaim.failed })
    }
  }

  return (
    <main className="mx-auto max-w-xl px-4 py-16 sm:px-6 sm:py-24">
      <div className="overflow-hidden rounded-2xl border border-border bg-card shadow-[var(--shadow-elevated)]">
        <div className="space-y-4 px-6 py-8 text-center sm:px-10">
          <p className="text-xs font-semibold tracking-wide text-primary uppercase">{COPY.giftClaim.eyebrow}</p>

          {view.status === 'loading' && <p className="text-sm text-muted-foreground">{COPY.giftClaim.loading}</p>}
          {view.status === 'invalid' && <p className="text-sm text-foreground">{COPY.giftClaim.invalid}</p>}
          {view.status === 'notYet' && <p className="text-sm text-foreground">{COPY.giftClaim.notYet}</p>}

          {view.status === 'ready' && (
            <>
              <h1 className="font-display text-2xl leading-tight font-semibold text-foreground">
                {view.receipt?.paperTitle ?? COPY.giftClaim.fallbackTitle}
              </h1>
              <p className="text-sm text-muted-foreground">{COPY.giftClaim.note}</p>
              {view.error && (
                <p role="alert" className="rounded-lg bg-muted px-3 py-2 text-sm text-foreground">
                  {view.error}
                </p>
              )}
              <button
                type="button"
                onClick={open}
                disabled={view.opening}
                aria-busy={view.opening}
                className="min-h-[52px] w-full rounded-xl bg-primary text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary-hover disabled:cursor-wait disabled:opacity-70"
              >
                {view.opening ? COPY.giftClaim.opening : COPY.giftClaim.open}
              </button>
              {view.receipt && (
                <a href={view.receipt.explorerUrl} target="_blank" rel="noreferrer" className="inline-block text-xs font-semibold text-primary hover:underline">
                  {COPY.giftClaim.explorer}
                </a>
              )}
            </>
          )}

          {view.status !== 'ready' && view.status !== 'loading' && (
            <Link href="/" className="inline-block text-sm font-semibold text-primary hover:underline">
              {COPY.giftClaim.home}
            </Link>
          )}
        </div>
      </div>
    </main>
  )
}
