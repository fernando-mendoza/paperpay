'use client'

import Link from 'next/link'
import { motion, useMotionValueEvent, useScroll } from 'motion/react'
import { useState } from 'react'
import { LogoMark } from '@/components/LogoMark'
import { COPY } from '@/lib/copy'

// Header mínimo de landing: logo + un solo CTA. Se oculta al bajar y vuelve al subir.
export function LandingHeader() {
  const { scrollY } = useScroll()
  const [hidden, setHidden] = useState(false)

  useMotionValueEvent(scrollY, 'change', (y) => {
    const previous = scrollY.getPrevious() ?? 0
    setHidden(y > previous && y > 120)
  })

  return (
    <motion.header
      animate={{ y: hidden ? '-100%' : '0%' }}
      transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
      className="fixed inset-x-0 top-0 z-30 border-b border-border/70 bg-background/85 px-4 backdrop-blur-md sm:px-6"
    >
      <div className="mx-auto flex h-16 max-w-7xl items-center justify-between gap-4">
        <Link href="/" className="flex items-center gap-2.5">
          <LogoMark size={28} />
          <span className="font-display text-lg leading-none font-semibold text-foreground">{COPY.brand.name}</span>
        </Link>
        <Link
          href="/"
          className="inline-flex min-h-[44px] items-center rounded-xl bg-foreground px-4 text-sm font-semibold whitespace-nowrap text-background transition-colors hover:bg-primary"
        >
          {COPY.landing.ctaPrimary}
        </Link>
      </div>
    </motion.header>
  )
}
