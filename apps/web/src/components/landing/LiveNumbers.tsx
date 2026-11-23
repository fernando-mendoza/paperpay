'use client'

import { animate, motion, useInView, useMotionValue, useReducedMotion, useTransform } from 'motion/react'
import { useEffect, useRef, useState } from 'react'
import { getEditorialStatsShared } from '@/services/editorial'
import { COPY } from '@/lib/copy'
import { Reveal } from './Reveal'

const LIVE = COPY.landing.live
const EASE = [0.22, 1, 0.36, 1] as const

interface Props {
  paperCount: number
  disciplineCount: number
}

type Reads = { status: 'loading' } | { status: 'error' } | { status: 'ok'; count: number; truncated: boolean; treasury: string }

// Números del producto, con el de lecturas leído en vivo de Horizon (pagos USDC a la tesorería).
// Los del catálogo salen del build. Todos cuentan desde 0 al entrar en pantalla.
export function LiveNumbers({ paperCount, disciplineCount }: Props) {
  const [reads, setReads] = useState<Reads>({ status: 'loading' })

  useEffect(() => {
    let cancelled = false
    getEditorialStatsShared()
      .then((s) => !cancelled && setReads({ status: 'ok', count: s.reads, truncated: s.truncated, treasury: s.treasury }))
      .catch(() => !cancelled && setReads({ status: 'error' }))
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <section className="px-4 py-24 sm:px-6 sm:py-28">
      <div className="mx-auto max-w-6xl">
        <Reveal className="mb-10 max-w-2xl">
          <p className="mb-3 flex items-center gap-2.5 text-[11px] font-semibold tracking-[0.2em] text-success-strong uppercase">
            <LiveDot />
            {LIVE.eyebrow}
          </p>
          <h2 className="mb-3 font-display text-[clamp(1.9rem,4vw,3.25rem)] leading-[1.05] font-semibold text-foreground">
            {LIVE.title}
          </h2>
          <p className="text-base leading-relaxed text-muted-foreground">{LIVE.subtitle}</p>
        </Reveal>

        <dl className="grid grid-cols-2 gap-px overflow-hidden rounded-3xl border border-border bg-border shadow-[var(--shadow-card)] lg:grid-cols-4">
          <Stat
            value={reads.status === 'ok' ? reads.count : null}
            suffix={reads.status === 'ok' && reads.truncated ? '+' : ''}
            failed={reads.status === 'error'}
            label={LIVE.reads}
            source={reads.status === 'error' ? LIVE.readsError : LIVE.readsSource}
            live
          />
          <Stat value={paperCount} label={LIVE.papers} source={LIVE.catalogSource} />
          <Stat value={disciplineCount} label={LIVE.disciplines} source={LIVE.catalogSource} />
          <Stat value={5} prefix="~" suffix=" s" label={LIVE.settle} source={LIVE.settleSource} />
        </dl>

        <div className="mt-5 flex flex-wrap items-center justify-between gap-3 text-sm">
          {reads.status === 'ok' ? (
            <a
              href={`https://stellar.expert/explorer/testnet/account/${reads.treasury}`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex min-h-[44px] items-center gap-1.5 font-semibold text-primary hover:underline"
            >
              {LIVE.verify}
              <span aria-hidden>↗</span>
            </a>
          ) : (
            <span />
          )}
          <span className="rounded-full bg-warning/15 px-3 py-1 text-xs font-semibold text-[#B45309]">{LIVE.testnet}</span>
        </div>
      </div>
    </section>
  )
}

function Stat({
  value,
  label,
  source,
  prefix = '',
  suffix = '',
  failed = false,
  live = false,
}: {
  value: number | null
  label: string
  source: string
  prefix?: string
  suffix?: string
  failed?: boolean
  live?: boolean
}) {
  return (
    <div className="flex flex-col bg-card p-5 sm:p-8">
      <dt className="order-2 mt-2 text-sm font-medium text-foreground">{label}</dt>
      <dd className="order-1 font-display text-[clamp(2.4rem,5vw,4rem)] leading-none font-semibold text-foreground tabular-nums">
        {value !== null ? (
          <Counter value={value} prefix={prefix} suffix={suffix} />
        ) : failed ? (
          '—'
        ) : (
          <span className="inline-block h-[0.8em] w-[1.6em] animate-pulse rounded-lg bg-muted align-middle" aria-label="Cargando" />
        )}
      </dd>
      <dd className="order-3 mt-3 flex items-center gap-1.5 text-xs text-muted-foreground">
        {live && !failed && <LiveDot small />}
        {source}
      </dd>
    </div>
  )
}

// El HTML del servidor ya trae el número final (sin JS se lee igual). Al entrar en pantalla
// vuelve a 0 y sube. Con reduced-motion se queda quieto.
function Counter({ value, prefix, suffix }: { value: number; prefix: string; suffix: string }) {
  const ref = useRef<HTMLSpanElement>(null)
  const inView = useInView(ref, { once: true, margin: '0px 0px -15% 0px' })
  const reduce = useReducedMotion()
  const count = useMotionValue(value)
  const text = useTransform(count, (v) => `${prefix}${Math.round(v).toLocaleString('es-MX')}${suffix}`)

  useEffect(() => {
    if (!inView) return
    if (reduce) {
      count.set(value)
      return
    }
    count.set(0)
    const controls = animate(count, value, { duration: 1.6, ease: EASE })
    return () => controls.stop()
  }, [inView, value, reduce, count])

  return <motion.span ref={ref}>{text}</motion.span>
}

function LiveDot({ small = false }: { small?: boolean }) {
  const size = small ? 'h-1.5 w-1.5' : 'h-2 w-2'
  return (
    <span className={`relative flex ${size}`} aria-hidden>
      <span className={`absolute inline-flex ${size} animate-ping rounded-full bg-success opacity-70 motion-reduce:animate-none`} />
      <span className={`relative inline-flex ${size} rounded-full bg-success`} />
    </span>
  )
}
