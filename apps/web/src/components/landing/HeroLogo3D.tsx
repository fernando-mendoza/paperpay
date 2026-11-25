'use client'

import type { MotionValue } from 'motion/react'
import { useEffect, useRef, useState } from 'react'
import { LogoMark } from '@/components/LogoMark'

interface Props {
  /** 0→1: reposo → capas separadas → se juntan con un giro (o, sin capas, dos medias vueltas). */
  progress: MotionValue<number>
  /** 1: las capas se separan con el scroll (escritorio). 0: el símbolo solo gira (celular). */
  layered: MotionValue<number>
  /** Etiqueta de cada capa (tinta, verde, azul) cuando están separadas. */
  labels: readonly [string, string, string]
  /** Sin animación: un solo cuadro fijo. */
  still: boolean
}

/** Colores de LogoMark: tinta, verde, azul. */
const LAYER_COLORS = ['#0F172A', '#10B981', '#2F5BFF'] as const

// El símbolo de PaperPay en three.js (la escena vive en logo3d-scene.ts). Mientras carga, o sin
// WebGL / sin JS, se ve el SVG de LogoMark. three llega en su propio chunk: no retrasa el titular.
export function HeroLogo3D({ progress, layered, labels, still }: Props) {
  const boxRef = useRef<HTMLDivElement>(null)
  const labelRefs = useRef<(HTMLSpanElement | null)[]>([])
  const [ready, setReady] = useState(false)

  useEffect(() => {
    const box = boxRef.current
    if (!box) return
    let disposed = false
    let scene: { dispose: () => void } | null = null

    import('./logo3d-scene')
      .then(({ createLogoScene }) => {
        if (disposed) return
        scene = createLogoScene({
          box,
          colors: LAYER_COLORS,
          labels: labelRefs.current,
          progress: () => progress.get(),
          layered: () => layered.get() === 1,
          still,
        })
        if (scene) setReady(true)
      })
      .catch(() => {}) // si el chunk no carga, se queda el SVG

    return () => {
      disposed = true
      scene?.dispose()
    }
  }, [progress, layered, still])

  return (
    <div ref={boxRef} className="relative h-full w-full">
      <div
        className="pointer-events-none absolute bottom-[8%] left-1/2 h-[9%] w-[46%] -translate-x-1/2 rounded-[50%] bg-foreground/15 blur-2xl"
        aria-hidden
      />
      <div
        className={`absolute inset-0 flex items-center justify-center transition-opacity duration-700 ${ready ? 'opacity-0' : 'opacity-100'}`}
        aria-hidden
      >
        <LogoMark size={220} className="h-[46%] w-[46%]" />
      </div>
      {labels.map((label, i) => (
        <span
          key={label}
          ref={(el) => {
            labelRefs.current[i] = el
          }}
          className="pointer-events-none absolute top-0 left-0 z-10 inline-flex items-center gap-2 rounded-full border border-border bg-card/90 px-3 py-1.5 text-xs font-semibold whitespace-nowrap text-foreground opacity-0 shadow-[var(--shadow-card)] backdrop-blur"
          aria-hidden
        >
          <span className="h-2 w-2 rounded-full" style={{ background: LAYER_COLORS[i] }} />
          {label}
        </span>
      ))}
    </div>
  )
}
