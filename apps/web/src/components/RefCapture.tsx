'use client'

import { useEffect } from 'react'
import { captureReferral } from '@/lib/referral'

/** Guarda el `?ref=` con el que se entró al sitio (cualquier página) y lo quita de la URL. */
export function RefCapture() {
  useEffect(() => {
    void captureReferral()
  }, [])
  return null
}
