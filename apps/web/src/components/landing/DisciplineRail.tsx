'use client'

import Link from 'next/link'
import {
  motion,
  useMotionValue,
  useMotionValueEvent,
  useScroll,
  useTransform,
  type MotionValue,
} from 'motion/react'
import { useEffect, useRef, useState } from 'react'
import { disciplineTheme } from '@/components/DisciplineCover'

export interface RailDiscipline {
  name: string
  count: number
  /** Título del artículo más reciente de la disciplina. */
  sample: string
}

interface Props {
  eyebrow: string
  title: string
  subtitle: string
  disciplines: readonly RailDiscipline[]
}

// Scroll vertical → carrusel horizontal. La sección mide una pantalla más lo que sobra del carril,
// así que al bajar el carril se desplaza de lado. La tarjeta que pasa por el centro se ilumina.
export function DisciplineRail({ eyebrow, title, subtitle, disciplines }: Props) {
  const sectionRef = useRef<HTMLElement>(null)
  const trackRef = useRef<HTMLDivElement>(null)
  const [distance, setDistance] = useState(0)
  const distanceMV = useMotionValue(0)
  const viewportW = useMotionValue(1280)

  useEffect(() => {
    const track = trackRef.current
    if (!track) return
    const measure = () => {
      const vw = document.documentElement.clientWidth
      const d = Math.max(0, track.offsetWidth - vw)
      viewportW.set(vw)
      distanceMV.set(d)
      setDistance(d)
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(track)
    window.addEventListener('resize', measure)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', measure)
    }
  }, [distanceMV, viewportW])

  const { scrollYProgress } = useScroll({ target: sectionRef, offset: ['start start', 'end end'] })
  const x = useTransform(() => -scrollYProgress.get() * distanceMV.get())
  const max = Math.max(...disciplines.map((d) => d.count))

  return (
    <section ref={sectionRef} className="relative bg-foreground text-white" style={{ height: `calc(100svh + ${distance}px)` }}>
      <div className="sticky top-0 flex h-[100svh] flex-col justify-center overflow-hidden">
        <div
          className="pointer-events-none absolute inset-x-0 top-0 h-40 bg-gradient-to-b from-foreground to-transparent"
          aria-hidden
        />
        <div className="relative mx-auto mb-8 w-full max-w-7xl px-4 sm:mb-12 sm:px-6">
          <p className="mb-3 flex items-center gap-3 text-[11px] font-semibold tracking-[0.2em] text-success uppercase">
            <span className="h-px w-6 bg-success" />
            {eyebrow}
          </p>
          <h2 className="mb-3 font-display text-[clamp(1.9rem,4vw,3.5rem)] leading-[1.05] font-semibold">{title}</h2>
          <p className="max-w-xl text-sm leading-relaxed text-slate-400 sm:text-base">{subtitle}</p>
        </div>

        <div data-rail className="relative overflow-hidden">
          <motion.div ref={trackRef} style={{ x }} className="flex w-max gap-5 py-6 pr-[8vw] pl-4 sm:gap-7 sm:pl-6 xl:pl-[calc((100vw-80rem)/2+1.5rem)]">
            {disciplines.map((discipline) => (
              <RailCard key={discipline.name} discipline={discipline} max={max} x={x} viewportW={viewportW} />
            ))}
          </motion.div>
        </div>
      </div>
    </section>
  )
}

function RailCard({
  discipline,
  max,
  x,
  viewportW,
}: {
  discipline: RailDiscipline
  max: number
  x: MotionValue<number>
  viewportW: MotionValue<number>
}) {
  // Se mide el envoltorio, no el <a>: con transform, Chrome toma el envoltorio como offsetParent
  // y el offsetLeft del enlace sale 0.
  const ref = useRef<HTMLDivElement>(null)
  const theme = disciplineTheme(discipline.name)
  // Distancia normalizada del centro de la tarjeta al centro de la pantalla (−1 … 1).
  // Los .get() van primero: useTransform se suscribe a los valores que lee en cada cálculo.
  const offset = useTransform(() => {
    const vw = viewportW.get()
    const shift = x.get()
    const el = ref.current
    if (!el) return 1
    const center = el.offsetLeft + shift + el.offsetWidth / 2
    return Math.max(-1, Math.min(1, (center - vw / 2) / (vw / 2)))
  })
  const rotateY = useTransform(offset, [-1, 0, 1], [16, 0, -16])
  const scale = useTransform(offset, [-1, 0, 1], [0.92, 1, 0.92])
  const [centered, setCentered] = useState(false)
  // Se ilumina solo la más cercana al centro: media tarjeta (más medio hueco) a cada lado.
  useMotionValueEvent(offset, 'change', (v) => {
    const el = ref.current
    if (!el) return
    const half = viewportW.get() / 2
    setCentered(Math.abs(v) * half < el.offsetWidth / 2 + 12)
  })

  // Inclinación de la pila de hojas con el puntero (solo puntero fino).
  const onPointerMove = (e: React.PointerEvent<HTMLAnchorElement>) => {
    if (e.pointerType !== 'mouse') return
    const r = e.currentTarget.getBoundingClientRect()
    e.currentTarget.style.setProperty('--ry', `${((e.clientX - r.left) / r.width - 0.5) * 18}deg`)
    e.currentTarget.style.setProperty('--rx', `${(0.5 - (e.clientY - r.top) / r.height) * 14 + 16}deg`)
  }
  const onPointerLeave = (e: React.PointerEvent<HTMLAnchorElement>) => {
    e.currentTarget.style.removeProperty('--ry')
    e.currentTarget.style.removeProperty('--rx')
  }

  return (
    <motion.div ref={ref} style={{ rotateY, scale, transformPerspective: 1400 }} className="flex-none">
      <Link
        href={`/?disciplina=${encodeURIComponent(discipline.name)}`}
        onPointerMove={onPointerMove}
        onPointerLeave={onPointerLeave}
        className={`group relative block w-[min(76vw,310px)] overflow-hidden rounded-[22px] border bg-[#0E1526] transition-[border-color,box-shadow] duration-500 ${
          centered ? 'border-white/25 shadow-[0_24px_70px_rgba(0,0,0,0.55)]' : 'border-white/8 shadow-[0_12px_40px_rgba(0,0,0,0.35)]'
        }`}
        style={{ boxShadow: centered ? `0 24px 70px rgba(0,0,0,0.55), 0 0 0 1px ${theme.from}55, 0 0 60px ${theme.from}33` : undefined }}
      >
        <div
          className={`pointer-events-none absolute inset-0 transition-opacity duration-500 ${centered ? 'opacity-100' : 'opacity-50'}`}
          style={{ background: `radial-gradient(90% 60% at 50% 18%, ${theme.from}40, transparent 70%)` }}
          aria-hidden
        />
        <div className="relative flex h-[190px] items-center justify-center sm:h-[210px]">
          <span className="absolute top-4 right-4 inline-flex items-center gap-1.5 rounded-full border border-white/15 bg-black/30 px-2.5 py-1 text-[11px] font-semibold text-white/90 backdrop-blur">
            <span className="h-1.5 w-1.5 rounded-full" style={{ background: theme.to }} />
            {discipline.count} artículos
          </span>
          <PaperStack discipline={discipline.name} from={theme.from} to={theme.to} />
        </div>

        <div className="relative px-5 pb-5">
          <div className="mb-3 flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="font-display text-xl font-semibold text-white">{discipline.name}</p>
              <p className="mt-0.5 line-clamp-1 text-xs text-slate-400">{discipline.sample}</p>
            </div>
            <span className="flex-none rounded-md border border-usdc/40 bg-usdc/15 px-2 py-0.5 text-[10px] font-bold text-[#7FB2EA]">USDC</span>
          </div>
          <p className="mb-3 flex items-baseline gap-1.5">
            <span className="font-display text-[2.6rem] leading-none font-semibold text-success">$0.50</span>
            <span className="text-sm text-slate-400">por artículo</span>
          </p>
          <div className="mb-3 h-1.5 overflow-hidden rounded-full bg-white/10">
            <span
              className="block h-full rounded-full"
              style={{ width: `${(discipline.count / max) * 100}%`, background: `linear-gradient(90deg, ${theme.from}, ${theme.to})` }}
            />
          </div>
          <div className="flex justify-between text-[11px] text-slate-400">
            <span>Resumen gratis</span>
            <span>Acceso 24 h</span>
          </div>
        </div>
      </Link>
    </motion.div>
  )
}

// Pila de hojas en falso 3D: la hoja de arriba es la portada de la disciplina y las de abajo,
// desplazadas, hacen el grosor. Se inclina con --rx/--ry (puntero) sobre una pose por defecto.
export function PaperStack({ discipline, from, to }: { discipline: string; from: string; to: string }) {
  const sheets = [6, 5, 4, 3, 2, 1]
  return (
    <div className="[perspective:900px]" aria-hidden>
      <div
        className="relative h-[124px] w-[100px] transition-transform duration-300 ease-out sm:h-[140px] sm:w-[112px]"
        style={{
          transform: 'rotateX(var(--rx, 16deg)) rotateY(var(--ry, -6deg)) rotateZ(-8deg)',
          transformStyle: 'preserve-3d',
        }}
      >
        <span className="absolute inset-0 translate-x-[14px] translate-y-[22px] rounded-md bg-black/50 blur-md" />
        {sheets.map((i) => (
          <span
            key={i}
            className="absolute inset-0 rounded-[5px] border border-black/10"
            style={{ transform: `translate(${i * 1.6}px, ${i * 2.2}px)`, background: i % 2 ? '#E9E4D9' : '#D8D1C3' }}
          />
        ))}
        <span className="absolute inset-0 overflow-hidden rounded-[5px] bg-[#FAF8F3] shadow-[0_1px_0_rgba(255,255,255,0.6)_inset]">
          <span className="block h-[44%] px-2 pt-2" style={{ background: `linear-gradient(135deg, ${from}, ${to})` }}>
            <span className="block truncate text-[7px] font-bold tracking-wider text-white/90 uppercase">{discipline}</span>
            <span className="mt-1 block h-[3px] w-8 rounded bg-white/50" />
          </span>
          <span className="block space-y-[5px] px-2 pt-2.5">
            {[90, 100, 76, 96, 64].map((w, j) => (
              <span key={j} className="block h-[3px] rounded bg-[#D9D3C7]" style={{ width: `${w}%` }} />
            ))}
          </span>
        </span>
      </div>
    </div>
  )
}
