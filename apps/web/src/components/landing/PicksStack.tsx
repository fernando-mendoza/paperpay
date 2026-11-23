'use client'

import Link from 'next/link'
import { motion, useReducedMotion, useScroll, useTransform, type MotionValue } from 'motion/react'
import { useEffect, useRef, useState } from 'react'
import { disciplineTheme } from '@/components/DisciplineCover'
import type { TopPick } from '@/lib/top-picks'
import { PaperStack } from './DisciplineRail'

interface Labels {
  why: string
  abstract: string
  cta: string
  access: string
}

interface Props {
  picks: readonly TopPick[]
  labels: Labels
}

// Tarjetas que se apilan con el scroll: cada una se queda fija arriba y la siguiente sube encima.
// Las de atrás se encogen y se oscurecen un poco, así que se ve el mazo crecer de 01 a 06.
// Sin JS el apilado sigue funcionando (es CSS sticky); solo se pierde el encogido.
export function PicksStack({ picks, labels }: Props) {
  const ref = useRef<HTMLDivElement>(null)
  const { scrollYProgress } = useScroll({ target: ref, offset: ['start start', 'end end'] })
  // La preferencia se lee después de montar: el HTML del servidor y el primer render deben coincidir.
  const prefersReduced = useReducedMotion()
  const [mounted, setMounted] = useState(false)
  useEffect(() => setMounted(true), [])
  const still = mounted && Boolean(prefersReduced)

  return (
    <div ref={ref} className="relative [--stack-step:10px] sm:[--stack-step:20px]">
      {picks.map((pick, i) => (
        <PickCard
          key={pick.id}
          pick={pick}
          index={i}
          total={picks.length}
          progress={scrollYProgress}
          still={still}
          labels={labels}
        />
      ))}
    </div>
  )
}

function PickCard({
  pick,
  index,
  total,
  progress,
  still,
  labels,
}: {
  pick: TopPick
  index: number
  total: number
  progress: MotionValue<number>
  still: boolean
  labels: Labels
}) {
  const theme = disciplineTheme(pick.discipline)
  const last = index === total - 1
  // La tarjeta i queda fija en i/(n−1) del progreso y la tapa la siguiente hasta (i+1)/(n−1).
  const span = Math.max(1, total - 1)
  const pinned = index / span
  const covered = Math.min(1, (index + 1) / span)
  const scale = useTransform(progress, last ? [0, 1] : [pinned, 1], still || last ? [1, 1] : [1, 1 - (total - 1 - index) * 0.04])
  const shade = useTransform(progress, last ? [0, 1] : [pinned, covered], last ? [0, 0] : [0, 0.22])
  const rank = String(pick.rank).padStart(2, '0')

  // Inclinación de la pila de hojas con el puntero (solo mouse).
  const onPointerMove = (e: React.PointerEvent<HTMLElement>) => {
    if (e.pointerType !== 'mouse') return
    const r = e.currentTarget.getBoundingClientRect()
    e.currentTarget.style.setProperty('--ry', `${((e.clientX - r.left) / r.width - 0.5) * 18}deg`)
    e.currentTarget.style.setProperty('--rx', `${(0.5 - (e.clientY - r.top) / r.height) * 14 + 16}deg`)
  }
  const onPointerLeave = (e: React.PointerEvent<HTMLElement>) => {
    e.currentTarget.style.removeProperty('--ry')
    e.currentTarget.style.removeProperty('--rx')
  }

  return (
    <div className="sticky top-0 flex h-[100svh] items-center justify-center px-4 sm:px-6">
      <motion.article
        data-pick
        onPointerMove={onPointerMove}
        onPointerLeave={onPointerLeave}
        style={{ scale, top: `calc(-2vh + ${index} * var(--stack-step))`, transformOrigin: '50% 0%' }}
        className="relative grid h-[min(560px,calc(100svh-120px))] w-full max-w-5xl grid-rows-[88px_minmax(0,1fr)] overflow-hidden rounded-[26px] border border-border bg-card shadow-[0_30px_80px_rgba(15,23,42,0.16)] sm:h-[min(500px,calc(100svh-120px))] sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)] sm:grid-rows-1"
      >
        <div
          className="relative overflow-hidden text-white"
          style={{ background: `linear-gradient(135deg, ${theme.from}, ${theme.to})` }}
        >
          <span className="pointer-events-none absolute -top-16 -right-16 h-56 w-56 rounded-full bg-white/10" aria-hidden />
          <span className="pointer-events-none absolute -bottom-20 -left-10 h-48 w-48 rounded-full bg-white/[0.07]" aria-hidden />
          <div className="relative flex h-full items-center gap-4 px-5 sm:flex-col sm:items-start sm:justify-between sm:p-8">
            <span
              className="font-display text-[3.25rem] leading-none font-semibold text-transparent sm:text-[clamp(5rem,9vw,8rem)]"
              style={{ WebkitTextStroke: '1.5px rgba(255,255,255,0.75)' }}
              aria-hidden
            >
              {rank}
            </span>
            <div className="hidden flex-1 items-center justify-center self-stretch sm:flex">
              <div className="scale-[1.35] lg:scale-[1.6]">
                <PaperStack discipline={pick.discipline} from={theme.from} to={theme.to} />
              </div>
            </div>
            <p className="text-xs font-semibold tracking-[0.18em] uppercase sm:text-sm">{pick.discipline}</p>
          </div>
        </div>

        <div className="flex min-h-0 flex-col p-5 sm:p-8">
          <p className="mb-2 text-xs text-muted-foreground">
            {pick.publisher} · {pick.date}
          </p>
          <h3 className="line-clamp-3 font-display text-[clamp(1.2rem,2.3vw,1.8rem)] leading-snug font-semibold text-foreground">
            {pick.title}
          </h3>
          <p className="mt-1.5 line-clamp-1 text-xs text-muted-foreground">{pick.authors}</p>

          <div className="mt-4 rounded-xl border-l-[3px] bg-muted px-4 py-3" style={{ borderColor: theme.from }}>
            <p className="mb-1 text-[11px] font-semibold tracking-wider uppercase" style={{ color: theme.from }}>
              {labels.why}
            </p>
            <p className="line-clamp-4 text-sm leading-relaxed text-foreground">{pick.why}</p>
          </div>

          <div className="mt-4 hidden lg:block">
            <p className="mb-1 text-[11px] font-semibold tracking-wider text-primary uppercase">{labels.abstract}</p>
            <p className="line-clamp-2 text-[13px] leading-relaxed text-muted-foreground">{pick.abstract}</p>
          </div>

          <div className="mt-auto flex flex-wrap items-end justify-between gap-3 pt-4">
            <div>
              <p className="font-display text-3xl leading-none font-semibold text-foreground">
                $0.50 <span className="font-sans text-sm font-medium text-muted-foreground">USDC</span>
              </p>
              <p className="mt-1 text-xs text-muted-foreground">{labels.access}</p>
            </div>
            <Link
              href={`/papers/${pick.id}`}
              aria-label={`${labels.cta}: ${pick.title}`}
              className="inline-flex min-h-[52px] w-full items-center justify-center gap-2 rounded-xl bg-primary px-6 text-sm font-semibold text-primary-foreground shadow-[var(--shadow-card)] transition-colors hover:bg-primary-hover sm:w-auto"
            >
              <LockIcon />
              {labels.cta}
            </Link>
          </div>
        </div>

        <motion.div className="pointer-events-none absolute inset-0 bg-foreground" style={{ opacity: shade }} aria-hidden />
      </motion.article>
    </div>
  )
}

function LockIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden>
      <rect x="2.5" y="7" width="11" height="7.5" rx="1.5" stroke="currentColor" strokeWidth="1.4" />
      <path d="M5.5 7V5.5a2.5 2.5 0 015 0V7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  )
}
