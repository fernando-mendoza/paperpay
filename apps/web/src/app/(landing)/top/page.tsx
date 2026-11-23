import type { Metadata } from 'next'
import Link from 'next/link'
import { HeroStats } from '@/components/HeroStats'
import { LandingFooter } from '@/components/landing/LandingFooter'
import { LandingHeader } from '@/components/landing/LandingHeader'
import { PicksFan } from '@/components/landing/PicksFan'
import { PicksStack } from '@/components/landing/PicksStack'
import { Reveal } from '@/components/landing/Reveal'
import { DISCIPLINE_COUNT, PAPER_COUNT } from '@/lib/catalog'
import { COPY } from '@/lib/copy'
import { resolveTopPicks } from '@/lib/top-picks'

const TOP = COPY.landing.top
const PICKS = resolveTopPicks()

export const metadata: Metadata = {
  title: TOP.metaTitle,
  description: TOP.metaDescription,
  openGraph: { title: TOP.metaTitle, description: TOP.metaDescription },
  twitter: { title: TOP.metaTitle, description: TOP.metaDescription },
}

// Landing B ("los elegidos") para campañas: seis artículos curados a mano, cada uno con el botón
// directo a su página de pago. Sin la historia larga de /lee: el anuncio vende el contenido.
export default function TopLandingPage() {
  return (
    <>
      <LandingHeader />
      <main>
        <section className="relative overflow-hidden">
          <div
            className="pointer-events-none absolute top-0 left-0 h-[620px] w-[620px] -translate-x-[35%] -translate-y-[30%] rounded-full opacity-10"
            style={{ background: 'radial-gradient(circle, #10B981 0%, transparent 70%)' }}
            aria-hidden
          />
          <div className="relative mx-auto grid min-h-[100svh] max-w-7xl grid-cols-1 items-center gap-10 px-4 pt-28 pb-16 sm:px-6 lg:grid-cols-[1.05fr_0.95fr]">
            <div className="max-w-xl">
              <div className="mb-6 inline-flex items-center gap-2 rounded-full bg-success-bg px-3 py-1.5 text-xs font-semibold text-success-strong">
                <span className="h-1.5 w-1.5 rounded-full bg-success" />
                {TOP.eyebrow}
              </div>
              <h1 className="mb-5 font-display text-[clamp(2.4rem,5.6vw,4.5rem)] leading-[1.05] font-semibold tracking-tight text-foreground">
                {TOP.headline} <span className="block text-primary">{TOP.price}</span>
              </h1>
              <p className="mb-8 text-lg leading-relaxed text-muted-foreground">{TOP.description}</p>
              <div className="flex flex-wrap items-center gap-3">
                <a
                  href="#seleccion"
                  className="inline-flex min-h-[52px] items-center rounded-xl bg-primary px-6 text-base font-semibold text-primary-foreground shadow-[var(--shadow-card)] transition-colors hover:bg-primary-hover"
                >
                  {TOP.ctaPicks}
                </a>
                <Link
                  href="/"
                  className="inline-flex min-h-[52px] items-center rounded-xl border border-border bg-card px-6 text-base font-semibold text-foreground transition-colors hover:border-border-strong"
                >
                  {TOP.closingCta}
                </Link>
              </div>
              <div className="mt-6">
                <HeroStats />
              </div>
            </div>
            <PicksFan papers={PICKS.slice(0, 3)} />
          </div>
        </section>

        <section id="seleccion" className="scroll-mt-4 pt-10 pb-24 sm:pt-16">
          <Reveal className="mx-auto mb-4 max-w-5xl px-4 sm:px-6">
            <p className="mb-3 flex items-center gap-3 text-[11px] font-semibold tracking-[0.2em] text-primary uppercase">
              <span className="h-px w-6 bg-primary" />
              {TOP.picks.eyebrow}
            </p>
            <h2 className="mb-3 font-display text-[clamp(1.9rem,4vw,3.25rem)] leading-[1.05] font-semibold text-foreground">
              {TOP.picks.title}
            </h2>
            <p className="max-w-xl text-base text-muted-foreground">{TOP.picks.subtitle}</p>
          </Reveal>
          <PicksStack picks={PICKS} labels={TOP.card} />
        </section>

        <section className="bg-foreground px-4 py-24 text-white sm:px-6 sm:py-28">
          <div className="mx-auto max-w-5xl">
            <Reveal>
              <p className="mb-3 flex items-center gap-3 text-[11px] font-semibold tracking-[0.2em] text-success uppercase">
                <span className="h-px w-6 bg-success" />
                {TOP.how.eyebrow}
              </p>
              <h2 className="mb-10 font-display text-[clamp(1.75rem,3.6vw,2.75rem)] leading-tight font-semibold">{TOP.how.title}</h2>
            </Reveal>
            <ol className="grid gap-4 sm:grid-cols-3 sm:gap-5">
              {TOP.how.steps.map((step, i) => (
                <li key={step.title}>
                  <Reveal delay={i * 0.1} className="h-full rounded-2xl border border-white/10 bg-white/[0.04] p-6">
                    <span className="mb-4 block font-mono text-sm text-success">{String(i + 1).padStart(2, '0')}</span>
                    <p className="mb-2 font-display text-xl font-semibold">{step.title}</p>
                    <p className="text-sm leading-relaxed text-slate-300">{step.description}</p>
                  </Reveal>
                </li>
              ))}
            </ol>
          </div>
        </section>

        <section className="px-4 py-24 sm:px-6 sm:py-32">
          <Reveal className="mx-auto max-w-3xl text-center">
            <h2 className="mb-3 font-display text-[clamp(2rem,4.5vw,3.25rem)] leading-tight font-semibold text-foreground">
              {TOP.closingTitle}
            </h2>
            <p className="mb-8 text-lg text-muted-foreground">
              Hay {PAPER_COUNT} artículos en {DISCIPLINE_COUNT} disciplinas, todos a $0.50 USDC.
            </p>
            <Link
              href="/"
              className="inline-flex min-h-[52px] items-center rounded-xl bg-primary px-7 text-base font-semibold text-primary-foreground shadow-[var(--shadow-card)] transition-colors hover:bg-primary-hover"
            >
              {TOP.closingCta}
            </Link>
          </Reveal>
        </section>
      </main>
      <LandingFooter />
    </>
  )
}
