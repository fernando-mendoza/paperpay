'use client'

import { useReducedMotion, type MotionValue } from 'motion/react'
import { useEffect, useRef, useState, type RefObject } from 'react'

interface Props {
  /** Progreso del paso de liquidación, 0→1. */
  progress: MotionValue<number>
  /** El paso de liquidación está en pantalla. */
  active: boolean
  /** Las cajas de la tarjeta de liquidación a donde caen las monedas. */
  publisherRef: RefObject<HTMLElement | null>
  treasuryRef: RefObject<HTMLElement | null>
}

// Capa 3D de monedas sobre la ventana de la historia del pago (la escena vive en coins3d-scene.ts).
// Se monta la primera vez que llega el paso de liquidación. Con reduced-motion, sin WebGL o sin JS no
// hay monedas: la tarjeta muestra el reparto igual, con sus números.
export function CoinRain({ progress, active, publisherRef, treasuryRef }: Props) {
  const boxRef = useRef<HTMLDivElement>(null)
  const activeRef = useRef(active)
  const reduced = useReducedMotion()
  const [started, setStarted] = useState(false)

  useEffect(() => {
    activeRef.current = active
    if (active) setStarted(true)
  }, [active])

  useEffect(() => {
    const box = boxRef.current
    if (!box || !started || reduced) return
    let disposed = false
    let scene: { dispose: () => void } | null = null

    import('./coins3d-scene')
      .then(({ createCoinScene }) => {
        if (disposed) return
        scene = createCoinScene({
          box,
          progress: () => progress.get(),
          active: () => activeRef.current,
          targets: () => ({ publisher: publisherRef.current, treasury: treasuryRef.current }),
        })
      })
      .catch(() => {}) // si el chunk no carga, la historia sigue sin monedas

    return () => {
      disposed = true
      scene?.dispose()
    }
  }, [started, reduced, progress, publisherRef, treasuryRef])

  return <div ref={boxRef} data-coins className="pointer-events-none absolute inset-0 z-30" aria-hidden />
}
