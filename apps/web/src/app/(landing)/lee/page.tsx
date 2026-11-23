import type { Metadata } from 'next'
import Link from 'next/link'
import { HeroPreview } from '@/components/HeroPreview'
import { HeroStats } from '@/components/HeroStats'
import { LandingHeader } from '@/components/landing/LandingHeader'
import { Manifesto } from '@/components/landing/Manifesto'
import { Reveal } from '@/components/landing/Reveal'
import { COPY } from '@/lib/copy'
import papers from '@/mocks/papers.json'

const L = COPY.landing
const LEE = L.lee

// Conteos del catálogo al compilar: no se escriben a mano para que no se desfasen.
const CATALOG = papers as { discipline?: string }[]
const PAPER_COUNT = CATALOG.length
const DISCIPLINE_COUNT = new Set(CATALOG.map((p) => p.discipline).filter(Boolean)).size

export const metadata: Metadata = {
  title: LEE.metaTitle,
  description: LEE.metaDescription,
  openGraph: { title: LEE.metaTitle, description: LEE.metaDescription },
  twitter: { title: LEE.metaTitle, description: LEE.metaDescription },
}

// Landing A ("la historia del pago") para campañas. Fase 1: hero estático, manifiesto y pasos.
export default function LeeLandingPage() {
  return (
    <>
      <LandingHeader />
      <main>
        <section className="relative overflow-hidden">
          <div
            className="pointer-events-none absolute top-0 right-0 h-[640px] w-[640px] translate-x-[30%] -translate-y-[25%] rounded-full opacity-10"
            style={{ background: 'radial-gradient(circle, #2F5BFF 0%, transparent 70%)' }}
            aria-hidden
          />
          <div className="relative mx-auto grid min-h-[100svh] max-w-7xl grid-cols-1 items-center gap-12 px-4 pt-28 pb-16 sm:px-6 lg:grid-cols-[1.05fr_0.95fr]">
            <div className="max-w-xl">
              <div className="mb-6 inline-flex items-center gap-2 rounded-full bg-primary-soft px-3 py-1.5 text-xs font-semibold text-primary">
                <span className="h-1.5 w-1.5 rounded-full bg-primary" />
                {L.badge}
              </div>
              <h1 className="mb-5 font-display text-[clamp(2.5rem,6vw,4.75rem)] leading-[1.05] font-semibold tracking-tight text-foreground">
                {LEE.headline} <span className="text-primary">{LEE.price}</span>
                <span className="block">{LEE.subhead}</span>
              </h1>
              <p className="mb-8 text-lg leading-relaxed text-muted-foreground">{LEE.description}</p>
              <div className="flex flex-wrap items-center gap-3">
                <Link
                  href="/"
                  className="inline-flex min-h-[52px] items-center rounded-xl bg-primary px-6 text-base font-semibold text-primary-foreground shadow-[var(--shadow-card)] transition-colors hover:bg-primary-hover"
                >
                  {L.ctaPrimary}
                </Link>
                <a
                  href="#como-funciona"
                  className="inline-flex min-h-[52px] items-center rounded-xl border border-border bg-card px-6 text-base font-semibold text-foreground transition-colors hover:border-border-strong"
                >
                  {L.ctaSecondary}
                </a>
              </div>
              <div className="mt-6">
                <HeroStats />
              </div>
            </div>
            <div className="animate-fade-in lg:pl-6">
              <HeroPreview />
            </div>
          </div>
        </section>

        <section className="px-4 py-28 sm:px-6 sm:py-40">
          <div className="mx-auto max-w-5xl">
            <Manifesto segments={LEE.manifesto} />
          </div>
        </section>

        <section id="como-funciona" className="scroll-mt-20 bg-foreground px-4 py-24 sm:px-6">
          <div className="mx-auto max-w-7xl">
            <Reveal>
              <h2 className="mb-2 text-center font-display text-3xl font-semibold text-white sm:text-4xl">{LEE.stepsTitle}</h2>
              <p className="mb-14 text-center text-subtle">{LEE.stepsSubtitle}</p>
            </Reveal>
            <ol className="grid grid-cols-1 gap-8 md:grid-cols-3">
              {LEE.steps.map((step, i) => (
                <li key={step.number}>
                  <Reveal delay={i * 0.12}>
                    <div className="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-primary/15">
                      <span className="font-mono text-lg font-semibold text-primary">{step.number}</span>
                    </div>
                    <h3 className="mb-3 font-display text-xl font-semibold text-white">{step.title}</h3>
                    <p className="text-sm leading-relaxed text-subtle">{step.description}</p>
                  </Reveal>
                </li>
              ))}
            </ol>
          </div>
        </section>

        <section className="px-4 py-24 sm:px-6 sm:py-32">
          <Reveal className="mx-auto max-w-3xl text-center">
            <h2 className="mb-3 font-display text-[clamp(2rem,4.5vw,3.25rem)] leading-tight font-semibold text-foreground">
              {PAPER_COUNT} artículos en {DISCIPLINE_COUNT} disciplinas
            </h2>
            <p className="mb-8 text-lg text-muted-foreground">{LEE.closingSubtitle}</p>
            <Link
              href="/"
              className="inline-flex min-h-[52px] items-center rounded-xl bg-primary px-7 text-base font-semibold text-primary-foreground shadow-[var(--shadow-card)] transition-colors hover:bg-primary-hover"
            >
              {L.ctaPrimary}
            </Link>
          </Reveal>
        </section>
      </main>
      <footer className="border-t border-border px-4 py-8 sm:px-6">
        <div className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-4 text-xs text-muted-foreground">
          <p>{L.footer}</p>
          <p>© 2026 {COPY.brand.name}</p>
        </div>
      </footer>
    </>
  )
}
