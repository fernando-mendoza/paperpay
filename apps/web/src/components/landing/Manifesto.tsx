'use client'

import { motion, useReducedMotion, useScroll, useTransform, type MotionValue } from 'motion/react'
import { useEffect, useRef, useState } from 'react'

export type ManifestoTone = 'old' | 'price' | 'accent'
export interface ManifestoSegment {
  text: string
  tone?: ManifestoTone
}

const TONE_CLASS: Record<ManifestoTone, string> = {
  old: 'text-muted-foreground line-through decoration-2',
  price: 'text-primary',
  accent: 'text-success',
}

interface Word {
  text: string
  tone?: ManifestoTone
}

// Una frase que se enciende palabra por palabra mientras se hace scroll (opacidad 0.14 → 1).
export function Manifesto({ segments }: { segments: readonly ManifestoSegment[] }) {
  const ref = useRef<HTMLParagraphElement>(null)
  // Se lee después de montar para que el primer render del cliente sea igual al HTML estático.
  const prefersReduced = useReducedMotion()
  const [mounted, setMounted] = useState(false)
  useEffect(() => setMounted(true), [])
  const reduce = mounted && Boolean(prefersReduced)
  const { scrollYProgress } = useScroll({ target: ref, offset: ['start 0.85', 'end 0.45'] })

  const words: Word[] = segments.flatMap((segment) =>
    segment.text.split(/\s+/).filter(Boolean).map((text) => ({ text, tone: segment.tone })),
  )

  return (
    <p
      ref={ref}
      className="font-display text-[clamp(1.75rem,4.2vw,3.5rem)] leading-[1.18] font-semibold tracking-tight text-foreground"
    >
      {words.map((word, i) => (
        <ManifestoWord
          key={i}
          progress={scrollYProgress}
          range={[i / words.length, (i + 1) / words.length]}
          still={reduce}
          className={word.tone ? TONE_CLASS[word.tone] : undefined}
        >
          {word.text}
        </ManifestoWord>
      ))}
    </p>
  )
}

function ManifestoWord({
  children,
  progress,
  range,
  still,
  className,
}: {
  children: string
  progress: MotionValue<number>
  range: [number, number]
  /** Con reduced-motion la palabra queda encendida y no sigue el scroll. */
  still: boolean
  className?: string
}) {
  const opacity = useTransform(progress, range, still ? [1, 1] : [0.14, 1])
  return (
    <>
      <motion.span data-manifesto-word style={{ opacity }} className={className}>
        {children}
      </motion.span>{' '}
    </>
  )
}
