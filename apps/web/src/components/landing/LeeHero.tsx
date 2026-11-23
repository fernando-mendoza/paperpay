'use client'

import Link from 'next/link'
import {
  AnimatePresence,
  motion,
  useMotionValue,
  useMotionValueEvent,
  useReducedMotion,
  useScroll,
  useTransform,
} from 'motion/react'
import { useEffect, useRef, useState } from 'react'
import { HeroStats } from '@/components/HeroStats'
import { COPY } from '@/lib/copy'
import { HeroLogo3D } from './HeroLogo3D'

const L = COPY.landing
const LEE = L.lee
const H = LEE.hero3d
const EASE = [0.22, 1, 0.36, 1] as const

// Hero de /lee con el símbolo en 3D.
// Escritorio: la sección se queda fija ~1.3 pantallas; al bajar, las tres "P" se separan como hojas
// (cada una con su paso del pago), se vuelven a juntar con un giro y el texto bajo el titular cambia.
// Móvil: sin fijar; el 3D va debajo del texto y se abre mientras cruza la pantalla.
// Reduced-motion: sección de una pantalla y un cuadro fijo. El titular siempre está en el HTML.
export function LeeHero() {
  const sectionRef = useRef<HTMLElement>(null)
  const stageRef = useRef<HTMLDivElement>(null)
  const prefersReduced = useReducedMotion()
  const [mounted, setMounted] = useState(false)
  useEffect(() => setMounted(true), [])
  const still = mounted && Boolean(prefersReduced)

  const desktop = useMotionValue(0)
  useEffect(() => {
    const mq = window.matchMedia('(min-width: 1024px)')
    const sync = () => desktop.set(mq.matches ? 1 : 0)
    sync()
    mq.addEventListener('change', sync)
    return () => mq.removeEventListener('change', sync)
  }, [desktop])

  const { scrollYProgress: pinned } = useScroll({ target: sectionRef, offset: ['start start', 'end end'] })
  const { scrollYProgress: passing } = useScroll({ target: stageRef, offset: ['start end', 'end start'] })
  // Los .get() van todos primero: useTransform se suscribe a lo que lee.
  const progress = useTransform(() => {
    const a = pinned.get()
    const b = passing.get()
    return desktop.get() ? a : Math.min(1, Math.max(0, (b - 0.2) / 0.6))
  })

  const [beat, setBeat] = useState(0)
  useMotionValueEvent(progress, 'change', (v) => {
    if (still) return
    const next = v < 0.28 ? 0 : v < 0.62 ? 1 : 2
    setBeat((current) => (current === next ? current : next))
  })
  const hintOpacity = useTransform(progress, [0, 0.05], [1, 0])

  return (
    <section ref={sectionRef} className="relative lg:h-[230vh] lg:motion-reduce:h-auto">
      <div className="relative overflow-hidden lg:sticky lg:top-0 lg:h-[100svh]">
        <div
          className="pointer-events-none absolute top-0 right-0 h-[640px] w-[640px] translate-x-[30%] -translate-y-[25%] rounded-full opacity-10"
          style={{ background: 'radial-gradient(circle, #2F5BFF 0%, transparent 70%)' }}
          aria-hidden
        />
        <div className="relative mx-auto grid min-h-[100svh] max-w-7xl grid-cols-1 items-center gap-2 px-4 pt-28 pb-10 sm:px-6 lg:grid-cols-[1fr_1fr] lg:gap-6 lg:pt-16 lg:pb-6">
          <div className="max-w-xl">
            <div className="mb-6 inline-flex items-center gap-2 rounded-full bg-primary-soft px-3 py-1.5 text-xs font-semibold text-primary">
              <span className="h-1.5 w-1.5 rounded-full bg-primary" />
              {L.badge}
            </div>
            <h1 className="mb-5 font-display text-[clamp(2.5rem,6vw,4.75rem)] leading-[1.05] font-semibold tracking-tight text-foreground">
              {LEE.headline} <span className="text-primary">{LEE.price}</span>
              <span className="block">{LEE.subhead}</span>
            </h1>
            <div className="relative mb-8 min-h-[5.25rem] sm:min-h-[4rem]">
              <AnimatePresence mode="wait" initial={false}>
                <motion.p
                  key={beat}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -8 }}
                  transition={{ duration: 0.35, ease: EASE }}
                  className="text-lg leading-relaxed text-muted-foreground"
                  aria-live="polite"
                >
                  {H.captions[beat]}
                </motion.p>
              </AnimatePresence>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <Link
                href="/"
                className="inline-flex min-h-[52px] items-center rounded-xl bg-primary px-6 text-base font-semibold text-primary-foreground shadow-[var(--shadow-card)] transition-colors hover:bg-primary-hover"
              >
                {L.ctaPrimary}
              </Link>
              <a
                href="#como-funciona"
                className="inline-flex min-h-[52px] items-center rounded-xl border border-border bg-card px-6 text-base font-semibold text-foreground transition-colors hover:border-border-strong"
              >
                {L.ctaSecondary}
              </a>
            </div>
            <div className="mt-6">
              <HeroStats />
            </div>
          </div>

          <div ref={stageRef} className="relative mx-auto aspect-square w-full max-w-[min(520px,100%)] lg:max-w-[min(640px,80svh)]">
            <HeroLogo3D progress={progress} labels={H.labels} still={still} />
          </div>
        </div>

        {!still && (
          <motion.div
            style={{ opacity: hintOpacity }}
            className="pointer-events-none absolute bottom-6 left-1/2 hidden -translate-x-1/2 flex-col items-center gap-2 text-[11px] font-semibold tracking-[0.2em] text-muted-foreground uppercase lg:flex"
            aria-hidden
          >
            {H.scrollHint}
            <span className="relative h-9 w-px overflow-hidden bg-border-strong/60">
              <span className="absolute inset-x-0 top-0 h-3 animate-[scrollCue_1.6s_ease-in-out_infinite] bg-foreground" />
            </span>
          </motion.div>
        )}
      </div>
    </section>
  )
}
