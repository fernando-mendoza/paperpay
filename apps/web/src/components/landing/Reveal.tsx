'use client'

import { motion } from 'motion/react'

const EASE = [0.22, 1, 0.36, 1] as const

interface Props {
  children: React.ReactNode
  className?: string
  /** Retraso en segundos, para escalonar elementos hermanos. */
  delay?: number
}

// Aparece al entrar en pantalla: sube, se enfoca y gana opacidad. Una sola vez.
// No usar en el hero: el titular tiene que estar visible en el HTML para el LCP.
export function Reveal({ children, className, delay = 0 }: Props) {
  return (
    <motion.div
      data-reveal
      className={className}
      initial={{ opacity: 0, y: 28, filter: 'blur(6px)' }}
      whileInView={{ opacity: 1, y: 0, filter: 'blur(0px)' }}
      viewport={{ once: true, margin: '0px 0px -12% 0px' }}
      transition={{ duration: 0.75, ease: EASE, delay }}
    >
      {children}
    </motion.div>
  )
}
