'use client'

import { AnimatePresence, motion, useMotionValueEvent, useScroll, useTransform, type MotionValue } from 'motion/react'
import { useRef, useState } from 'react'
import { useLenis } from './LandingShell'
import { StoryWindow, type StoryPaper } from './StoryWindow'

export interface StoryChip {
  label: string
  value: string
}
export interface StoryStep {
  title: string
  description: string
  chips: readonly StoryChip[]
}

interface Props {
  eyebrow: string
  title: string
  steps: readonly StoryStep[]
  paper: StoryPaper
}

const EASE = [0.22, 1, 0.36, 1] as const
// Alto de scroll por paso (vh). La sección mide pasos × esto + una pantalla.
const VH_PER_STEP = 85

// "Cómo funciona" contado con scroll: la sección se queda fija, a la izquierda se enciende el paso
// activo (su barra se llena con el scroll) y a la derecha la ventana del navegador cambia de estado.
export function PaymentStory({ eyebrow, title, steps, paper }: Props) {
  const ref = useRef<HTMLElement>(null)
  const lenis = useLenis()
  const n = steps.length
  const { scrollYProgress } = useScroll({ target: ref, offset: ['start start', 'end end'] })
  const [active, setActive] = useState(0)

  useMotionValueEvent(scrollYProgress, 'change', (v) => {
    const next = Math.min(n - 1, Math.max(0, Math.floor(v * n)))
    setActive((current) => (current === next ? current : next))
  })

  const rotateY = useTransform(scrollYProgress, [0, 1], [-10, -3])

  const goTo = (index: number) => {
    const el = ref.current
    if (!el) return
    const top = el.getBoundingClientRect().top + window.scrollY
    const target = top + (el.offsetHeight - window.innerHeight) * ((index + 0.3) / n)
    if (lenis) lenis.scrollTo(target, { duration: 1.1 })
    else window.scrollTo({ top: target })
  }

  const chips = steps[active].chips

  return (
    <section
      ref={ref}
      id="como-funciona"
      className="relative bg-foreground text-white"
      style={{ height: `${n * VH_PER_STEP + 100}vh` }}
    >
      <div className="sticky top-0 flex h-[100svh] items-center overflow-hidden">
        <Backdrop />

        <div className="relative mx-auto grid w-full max-w-7xl grid-cols-1 items-center gap-5 px-4 sm:px-6 lg:grid-cols-[0.85fr_1.15fr] lg:gap-14">
          <div className="order-2 lg:order-1">
            <Heading eyebrow={eyebrow} title={title} className="hidden lg:block" />
            <ol className="space-y-1.5">
              {steps.map((step, i) => (
                <StepCard
                  key={step.title}
                  index={i}
                  steps={n}
                  step={step}
                  active={i === active}
                  progress={scrollYProgress}
                  onSelect={() => goTo(i)}
                />
              ))}
            </ol>
          </div>

          <div className="order-1 lg:order-2">
            <Heading eyebrow={eyebrow} title={title} className="lg:hidden" />
            <div className="relative">
              <AnimatePresence mode="popLayout" initial={false}>
                <motion.span
                  key={active}
                  initial={{ opacity: 0, y: 60 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -60 }}
                  transition={{ duration: 0.6, ease: EASE }}
                  className="pointer-events-none absolute -top-[0.58em] right-0 hidden font-display text-[clamp(9rem,15vw,15rem)] leading-none font-semibold text-transparent select-none sm:block"
                  style={{ WebkitTextStroke: '1.5px rgba(255,255,255,0.14)' }}
                  aria-hidden
                >
                  {String(active + 1).padStart(2, '0')}
                </motion.span>
              </AnimatePresence>

              <motion.div style={{ rotateY, rotateX: 3, transformPerspective: 1800 }} className="relative">
                <StoryWindow active={active} progress={scrollYProgress} steps={n} paper={paper} />
              </motion.div>

              <AnimatePresence mode="popLayout" initial={false}>
                <Chip key={`a-${active}`} chip={chips[0]} className="-top-4 left-2 sm:-top-6 lg:-left-10" />
                <Chip key={`b-${active}`} chip={chips[1]} className="right-2 -bottom-5 sm:-bottom-7 lg:-right-8" delay={0.08} />
              </AnimatePresence>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

function Heading({ eyebrow, title, className }: { eyebrow: string; title: string; className?: string }) {
  return (
    <div className={className}>
      <p className="mb-2 flex items-center gap-3 text-[11px] font-semibold tracking-[0.2em] text-success uppercase lg:mb-3">
        <span className="h-px w-6 bg-success" />
        {eyebrow}
      </p>
      <h2 className="mb-6 font-display text-[clamp(1.6rem,3.6vw,3.25rem)] leading-[1.08] font-semibold lg:mb-8">{title}</h2>
    </div>
  )
}

function StepCard({
  index,
  steps,
  step,
  active,
  progress,
  onSelect,
}: {
  index: number
  steps: number
  step: StoryStep
  active: boolean
  progress: MotionValue<number>
  onSelect: () => void
}) {
  // Pasos previos: barra llena. Activo: se llena con el scroll. Siguientes: vacía.
  const fill = useTransform(progress, (v) => Math.min(1, Math.max(0, v * steps - index)))

  return (
    <li data-step className={active ? 'block' : 'hidden lg:block'}>
      <button
        type="button"
        onClick={onSelect}
        aria-current={active ? 'step' : undefined}
        className={`relative w-full overflow-hidden rounded-2xl border px-5 py-4 text-left transition-colors duration-500 ${
          active ? 'border-white/12 bg-white/[0.06]' : 'border-transparent hover:bg-white/[0.03]'
        }`}
      >
        <span className="absolute inset-x-0 top-0 h-px bg-white/10" aria-hidden />
        <motion.span
          className="absolute top-0 left-0 h-[2px] w-full origin-left bg-gradient-to-r from-primary to-success"
          style={{ scaleX: fill }}
          aria-hidden
        />
        <span className="flex gap-4">
          <span className={`pt-1 font-mono text-sm transition-colors duration-500 ${active ? 'text-success' : 'text-success/40'}`}>
            {String(index + 1).padStart(2, '0')}
          </span>
          <span>
            <span
              className={`block font-display text-lg font-semibold transition-colors duration-500 sm:text-xl ${
                active ? 'text-white' : 'text-white/35'
              }`}
            >
              {step.title}
            </span>
            <span
              className={`mt-1 block text-sm leading-relaxed transition-colors duration-500 ${
                active ? 'text-slate-300' : 'text-white/25'
              }`}
            >
              {step.description}
            </span>
          </span>
        </span>
      </button>
    </li>
  )
}

function Chip({ chip, className, delay = 0 }: { chip: StoryChip; className: string; delay?: number }) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 14, scale: 0.94 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -10, scale: 0.96 }}
      transition={{ duration: 0.5, ease: EASE, delay }}
      className={`absolute z-30 rounded-xl border border-white/15 bg-[#0B1220]/85 px-3.5 py-2.5 shadow-[0_12px_40px_rgba(0,0,0,0.45)] backdrop-blur-md sm:px-4 sm:py-3 ${className}`}
    >
      <p className="text-[10px] font-semibold tracking-[0.16em] text-slate-400 uppercase">{chip.label}</p>
      <p className="font-display text-base font-semibold text-success sm:text-lg">{chip.value}</p>
    </motion.div>
  )
}

function Backdrop() {
  return (
    <>
      <div
        className="pointer-events-none absolute inset-0 opacity-100"
        style={{
          backgroundImage:
            'linear-gradient(rgba(255,255,255,0.035) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.035) 1px, transparent 1px)',
          backgroundSize: '56px 56px',
          maskImage: 'radial-gradient(ellipse at 60% 50%, black 30%, transparent 75%)',
          WebkitMaskImage: 'radial-gradient(ellipse at 60% 50%, black 30%, transparent 75%)',
        }}
        aria-hidden
      />
      <div
        className="pointer-events-none absolute top-1/2 right-[10%] h-[620px] w-[620px] -translate-y-1/2 rounded-full opacity-30 blur-3xl"
        style={{ background: 'radial-gradient(circle, #2F5BFF 0%, transparent 65%)' }}
        aria-hidden
      />
      <div
        className="pointer-events-none absolute right-[30%] bottom-0 h-[380px] w-[380px] rounded-full opacity-15 blur-3xl"
        style={{ background: 'radial-gradient(circle, #10B981 0%, transparent 65%)' }}
        aria-hidden
      />
    </>
  )
}
