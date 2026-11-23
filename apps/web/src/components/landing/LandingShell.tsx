'use client'

import Lenis from 'lenis'
import { MotionConfig, useReducedMotion } from 'motion/react'
import { createContext, useContext, useEffect, useState } from 'react'

const LenisContext = createContext<Lenis | null>(null)

/** Instancia de Lenis de la landing; `null` con reduced-motion o antes de montar. */
export function useLenis(): Lenis | null {
  return useContext(LenisContext)
}

// Scroll suave (Lenis) solo en las landings. Lenis mueve el scroll nativo, así que los
// `useScroll` de motion siguen funcionando. Con `prefers-reduced-motion` no se crea Lenis y
// MotionConfig apaga los movimientos de transform.
export function LandingShell({ children }: { children: React.ReactNode }) {
  const reduce = useReducedMotion()
  const [lenis, setLenis] = useState<Lenis | null>(null)

  useEffect(() => {
    if (reduce) return
    const instance = new Lenis({ autoRaf: true, lerp: 0.1, wheelMultiplier: 0.85, anchors: { offset: -72 } })
    setLenis(instance)
    return () => {
      instance.destroy()
      setLenis(null)
    }
  }, [reduce])

  return (
    <MotionConfig reducedMotion="user">
      <LenisContext.Provider value={lenis}>{children}</LenisContext.Provider>
    </MotionConfig>
  )
}
