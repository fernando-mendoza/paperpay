'use client'

import { motion } from 'motion/react'
import { DisciplineCover } from '@/components/DisciplineCover'

export interface FanPaper {
  rank: number
  discipline: string
  title: string
}

const EASE = [0.22, 1, 0.36, 1] as const

// Posición de cada carta del abanico: la del centro al frente, las de los lados giradas y atrás.
const POSES = [
  'z-10 left-[6%] translate-y-8 -rotate-[8deg] sm:left-0',
  'z-20 left-1/2 -translate-x-1/2',
  'z-10 right-[6%] translate-y-8 rotate-[8deg] sm:right-0',
] as const

// Abanico de tres artículos de la selección para el hero de /top.
// El giro va en el contenedor (CSS fijo) y la entrada en el hijo: sin JS, el <noscript> del layout
// solo anula el transform de la entrada y el abanico conserva su forma.
export function PicksFan({ papers }: { papers: readonly FanPaper[] }) {
  return (
    <div className="relative mx-auto h-[290px] w-full max-w-[380px] sm:h-[400px] sm:max-w-[460px] lg:h-[440px] lg:max-w-[540px]" aria-hidden>
      {papers.slice(0, 3).map((paper, i) => (
        <div
          key={paper.title}
          className={`absolute top-4 w-[150px] origin-bottom sm:w-[220px] lg:w-[250px] ${POSES[i] ?? POSES[1]}`}
        >
          <motion.div
            data-reveal
            initial={{ opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.8, ease: EASE, delay: 0.15 + (i === 1 ? 0.2 : i * 0.08) }}
            className="overflow-hidden rounded-2xl border border-border bg-card shadow-[0_24px_60px_rgba(15,23,42,0.18)]"
          >
            <div className="relative">
              <DisciplineCover discipline={paper.discipline} className="h-[60px] w-full sm:h-[88px] lg:h-[100px]" />
              <span className="absolute right-3 bottom-2 font-display text-2xl leading-none font-semibold text-white/85">
                {String(paper.rank).padStart(2, '0')}
              </span>
            </div>
            <div className="p-3.5 sm:p-4">
              <p className="mb-3 line-clamp-4 font-display text-sm leading-snug font-semibold text-foreground sm:text-base">
                {paper.title}
              </p>
              <div className="flex items-center justify-between text-[10px] sm:text-[11px]">
                <span className="rounded-full bg-usdc-bg px-2 py-0.5 font-semibold whitespace-nowrap text-usdc">0.50 USDC</span>
                <span className="hidden whitespace-nowrap text-muted-foreground sm:inline">Resumen gratis</span>
              </div>
            </div>
          </motion.div>
        </div>
      ))}
    </div>
  )
}
