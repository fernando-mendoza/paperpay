'use client'

import Link from 'next/link'
import {
  AnimatePresence,
  motion,
  type MotionValue,
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
const C = H.cinema
const EASE = [0.22, 1, 0.36, 1] as const

// Tramos del scroll en el celular (0→1 de la sección fija). Cada escena tiene una meseta en la que
// se ve completa, para que donde se detenga el dedo se lea una escena y no una transición.
const M = {
  out1: [0.1, 0.24], // sale la escena 1
  scene2: [0.24, 0.34, 0.54, 0.62], // entra, se queda y sale la escena 2
  in3: [0.66, 0.76], // entra la escena 3 (cada línea un poco después)
  // Dónde está el centro del símbolo (desde el centro de la pantalla) y su escala, por escena.
  stageAt: [0, 0.1, 0.3, 0.56, 0.72, 1],
  stageY: ['-22svh', '-22svh', '-4svh', '-4svh', '-24svh', '-24svh'],
  stageScale: [1, 1, 0.52, 0.52, 0.7, 0.7],
}

// Hero de /lee con el símbolo en 3D. La sección se queda fija y el scroll cuenta una historia:
// - Escritorio (sin cambios): ~1.3 pantallas; las tres "P" se separan como hojas (cada una con su
//   paso del pago), se juntan con un giro y el texto bajo el titular cambia en 3 tramos.
// - Celular: 2 pantallas y tres escenas. (1) símbolo arriba y el titular abajo; (2) "Lee [símbolo]
//   por $0.50" con el símbolo chico entre las dos líneas y dos datos; (3) el símbolo sube y quedan
//   "Sin suscripción. / Sin cuenta. / Solo pagas lo que lees." Entre escenas el símbolo da una vuelta.
//   El lado de la caja del símbolo es `--s`; las escenas 2 y 3 se separan del centro con esa medida.
//   En pantallas bajas (Safari con barras: ~664 px en un iPhone, ~548 en un SE) se quita el dato en
//   vivo y, por debajo de 620 px, también el subtítulo, y el titular se achica.
// - Reduced-motion: una pantalla y un cuadro fijo (la escena 1). El titular siempre está en el HTML.
// La entrada (palabras del titular que suben y el resto que aparece) es CSS: ver LandingIntro.
export function LeeHero() {
  const sectionRef = useRef<HTMLElement>(null)
  const prefersReduced = useReducedMotion()
  const [mounted, setMounted] = useState(false)
  useEffect(() => setMounted(true), [])
  const still = mounted && Boolean(prefersReduced)

  // 1 en escritorio (≥1024 px), 0 en el celular. Antes de montar vale 0: el SSR es el del celular.
  const desktop = useMotionValue(0)
  useEffect(() => {
    const mq = window.matchMedia('(min-width: 1024px)')
    const sync = () => desktop.set(mq.matches ? 1 : 0)
    sync()
    mq.addEventListener('change', sync)
    return () => mq.removeEventListener('change', sync)
  }, [desktop])

  // Con reduced-motion el progreso queda en 0 (escena 1): la sección mide una pantalla y, sin esto,
  // su progreso podría leerse como 1. Los `.get()` van antes del `if`: useTransform se suscribe a lo que lee.
  const { scrollYProgress: scrolled } = useScroll({ target: sectionRef, offset: ['start start', 'end end'] })
  const frozen = useMotionValue(0)
  useEffect(() => frozen.set(still ? 1 : 0), [still, frozen])
  const progress = useTransform(() => {
    const isFrozen = frozen.get()
    const value = scrolled.get()
    return isFrozen ? 0 : value
  })

  const [beat, setBeat] = useState(0)
  useMotionValueEvent(progress, 'change', (v) => {
    if (still) return
    // El texto bajo el titular cambia solo en escritorio; en el celular lo cuentan las escenas.
    const next = !desktop.get() ? 0 : v < 0.28 ? 0 : v < 0.62 ? 1 : 2
    setBeat((current) => (current === next ? current : next))
  })
  const hintOpacity = useTransform(progress, [0, 0.05], [1, 0])

  // Valores del celular. En escritorio los anulan clases `lg:…!` (important): así no dependen de saber el
  // ancho antes de hidratar (el SSR no lo sabe) y el símbolo no salta en el primer cuadro.
  const scene1Opacity = useTransform(progress, M.out1, [1, 0])
  const scene1Y = useTransform(progress, M.out1, [0, -48])
  const scene1Pointer = useTransform(scene1Opacity, (o): 'none' | 'auto' => (o < 0.05 ? 'none' : 'auto'))
  const stageY = useTransform(progress, M.stageAt, M.stageY)
  const stageScale = useTransform(progress, M.stageAt, M.stageScale)
  const scene2Opacity = useTransform(progress, M.scene2, [0, 1, 1, 0])
  const scene2Y = useTransform(progress, M.scene2, [28, 0, 0, -28])
  const scene3Opacity = useTransform(progress, M.in3, [0, 1])
  const scene3Y = useTransform(progress, M.in3, [28, 0])

  let word = 0
  const words = (text: string, className?: string) =>
    text.split(' ').map((w) => {
      const i = word++
      return (
        <span key={`${i}-${w}`} className={`-mt-[0.08em] -mb-[0.14em] inline-block overflow-hidden pt-[0.08em] pb-[0.14em] align-bottom ${className ?? ''}`}>
          <span className="lee-word" style={{ '--i': i } as React.CSSProperties}>
            {w}
          </span>
        </span>
      )
    })
  const space = ' '

  return (
    <section ref={sectionRef} data-cinema className="relative h-[300vh] motion-reduce:h-auto lg:h-[230vh] lg:motion-reduce:h-auto">
      <div className="sticky top-0 h-[100svh] overflow-hidden [--s:min(68vw,36svh,340px)] [@media(max-height:620px)]:[--s:min(68vw,30svh)]">
        <div
          className="pointer-events-none absolute top-0 right-0 h-[640px] w-[640px] translate-x-[30%] -translate-y-[25%] rounded-full opacity-10"
          style={{ background: 'radial-gradient(circle, #2F5BFF 0%, transparent 70%)' }}
          aria-hidden
        />
        <div className="relative mx-auto flex h-full max-w-7xl flex-col justify-end px-4 pt-20 pb-[max(1.75rem,env(safe-area-inset-bottom))] sm:px-6 lg:grid lg:grid-cols-[1fr_1fr] lg:items-center lg:justify-normal lg:gap-6 lg:pt-16 lg:pb-6">
          <motion.div style={{ opacity: scene1Opacity, y: scene1Y, pointerEvents: scene1Pointer }} className="relative z-10 max-w-xl lg:pointer-events-auto! lg:transform-none! lg:opacity-100!">
            <div
              className="lee-after-intro mb-4 inline-flex items-center gap-2 rounded-full bg-primary-soft px-3 py-1.5 text-xs font-semibold text-primary lg:mb-6"
              style={{ '--d': '0.95s' } as React.CSSProperties}
            >
              <span className="h-1.5 w-1.5 rounded-full bg-primary" />
              {L.badge}
            </div>
            <h1 className="mb-4 font-display text-[clamp(2.25rem,6vw,4.75rem)] max-lg:[@media(max-height:620px)]:text-[1.9rem] leading-[1.05] font-semibold tracking-tight text-foreground lg:mb-5">
              {words(LEE.headline).flatMap((w, i) => (i ? [space, w] : [w]))}
              {space}
              {words(LEE.price, 'text-primary')}
              <span className="block">{words(LEE.subhead).flatMap((w, i) => (i ? [space, w] : [w]))}</span>
            </h1>
            <div
              className="lee-after-intro relative mb-6 min-h-[4.5rem] sm:min-h-[4rem] lg:mb-8 lg:min-h-[5.25rem] max-lg:[@media(max-height:620px)]:hidden"
              style={{ '--d': '1.3s' } as React.CSSProperties}
            >
              <AnimatePresence mode="wait" initial={false}>
                <motion.p
                  key={beat}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -8 }}
                  transition={{ duration: 0.35, ease: EASE }}
                  className="text-base leading-relaxed text-muted-foreground sm:text-lg"
                  aria-live="polite"
                >
                  {H.captions[beat]}
                </motion.p>
              </AnimatePresence>
            </div>
            <div className="lee-after-intro flex items-center gap-3" style={{ '--d': '1.4s' } as React.CSSProperties}>
              <Link
                href="/"
                className="inline-flex min-h-[48px] items-center rounded-xl bg-primary px-4 text-[15px] font-semibold whitespace-nowrap text-primary-foreground shadow-[var(--shadow-card)] transition-colors hover:bg-primary-hover sm:px-6 lg:min-h-[52px] lg:text-base"
              >
                {L.ctaPrimary}
              </Link>
              <a
                href="#como-funciona"
                className="inline-flex min-h-[48px] items-center rounded-xl border border-border bg-card px-4 text-[15px] font-semibold whitespace-nowrap text-foreground transition-colors hover:border-border-strong sm:px-6 lg:min-h-[52px] lg:text-base"
              >
                <span className="lg:hidden">{C.ctaSecondaryShort}</span>
                <span className="hidden lg:inline">{L.ctaSecondary}</span>
              </a>
            </div>
            <div className="lee-after-intro mt-5 lg:mt-6 max-lg:[@media(max-height:740px)]:hidden" style={{ '--d': '1.5s' } as React.CSSProperties}>
              <HeroStats />
            </div>
          </motion.div>

          {/* Celular: capa sobre toda la pantalla y el símbolo se mueve con transform. Escritorio: columna del grid. */}
          <div
            className="lee-after-intro pointer-events-none absolute inset-0 flex items-center justify-center lg:pointer-events-auto lg:relative lg:inset-auto lg:block"
            style={{ '--d': '0.9s' } as React.CSSProperties}
          >
            <motion.div
              style={{ y: stageY, scale: stageScale }}
              className="relative aspect-square w-(--s) lg:mx-auto lg:w-full lg:max-w-[min(640px,80svh)] lg:transform-none!"
            >
              <HeroLogo3D progress={progress} layered={desktop} labels={H.labels} still={still} />
            </motion.div>
          </div>
        </div>

        {/* Escena 2 (solo celular): "Lee" arriba del símbolo y "por $0.50" abajo. El símbolo, a escala
            0.52, mide 0.52·--s: las líneas se separan del centro (46%) la mitad de eso. */}
        <motion.div style={{ opacity: scene2Opacity }} className="pointer-events-none absolute inset-0 lg:hidden">
          <motion.p
            style={{ y: scene2Y }}
            className="absolute inset-x-0 bottom-[calc(54%+var(--s)*0.26+0.25rem)] text-center font-display text-[clamp(3rem,15vw,4.5rem)] leading-none font-semibold text-foreground"
          >
            {C.lead}
          </motion.p>
          <motion.div style={{ y: scene2Y }} className="absolute inset-x-0 top-[calc(46%+var(--s)*0.26+0.25rem)] px-4 text-center">
            <p className="font-display text-[clamp(3rem,15vw,4.5rem)] leading-none font-semibold text-foreground">
              {C.tail} <span className="text-primary">{LEE.price}</span>
            </p>
            <p className="mx-auto mt-4 max-w-xs text-[15px] leading-relaxed text-muted-foreground">{C.scene2}</p>
            <div className="mx-auto mt-5 grid max-w-sm grid-cols-2 gap-3 text-left">
              {C.facts.map((fact) => (
                <div key={fact.label} className="rounded-xl border border-border bg-card px-3.5 py-3 shadow-[var(--shadow-card)]">
                  <p className="font-display text-2xl leading-none font-semibold text-foreground">{fact.value}</p>
                  <p className="mt-1.5 text-xs text-muted-foreground">{fact.label}</p>
                </div>
              ))}
            </div>
          </motion.div>
        </motion.div>

        {/* Escena 3 (solo celular): el símbolo arriba (26%, escala 0.7) y el texto debajo. */}
        <motion.div
          style={{ opacity: scene3Opacity, y: scene3Y }}
          className="pointer-events-none absolute inset-x-0 top-[calc(26%+var(--s)*0.35+1.5rem)] px-4 lg:hidden"
        >
          {C.lines.map((line, i) => (
            <SceneLine key={line} progress={progress} index={i} strong={i === C.lines.length - 1}>
              {line}
            </SceneLine>
          ))}
          <p className="mt-5 max-w-sm text-[15px] leading-relaxed text-muted-foreground">{C.scene3}</p>
          <p className="mt-4 max-w-sm rounded-xl border border-border bg-card px-3.5 py-2.5 text-[13px] leading-snug text-muted-foreground">
            {C.mobileNote}
          </p>
        </motion.div>

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

/** Una línea de la escena 3: entra un poco después que la anterior. La última va en tinta, las otras tenues. */
function SceneLine({ progress, index, strong, children }: { progress: MotionValue<number>; index: number; strong: boolean; children: string }) {
  const start = M.in3[0] + index * 0.035
  const opacity = useTransform(progress, [start, start + 0.08], [0, 1])
  const y = useTransform(progress, [start, start + 0.08], [16, 0])
  return (
    <motion.p
      style={{ opacity, y }}
      className={`font-display text-[clamp(2rem,9.5vw,2.75rem)] leading-[1.1] font-semibold tracking-tight ${strong ? 'text-foreground' : 'text-foreground/30'}`}
    >
      {children}
    </motion.p>
  )
}
