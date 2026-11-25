import type { Metadata } from 'next'
import 'lenis/dist/lenis.css'
import { LandingShell } from '@/components/landing/LandingShell'

// Landings para campañas. No compiten con el catálogo en buscadores: noindex, pero se siguen los links.
export const metadata: Metadata = {
  robots: { index: false, follow: true },
}

const NO_JS_CSS =
  '[data-reveal],[data-manifesto-word]{opacity:1!important;transform:none!important;filter:none!important}[data-step]{display:block!important}[data-rail]{overflow-x:auto!important}[data-cinema]{height:auto!important}'

export default function LandingLayout({ children }: { children: React.ReactNode }) {
  return (
    <LandingShell>
      {/* Sin JS, motion deja los bloques en su estado inicial (opacidad 0): se fuerzan visibles. */}
      <noscript>
        <style>{NO_JS_CSS}</style>
      </noscript>
      {children}
    </LandingShell>
  )
}
