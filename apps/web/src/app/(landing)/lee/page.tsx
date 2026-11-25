import type { Metadata } from 'next'
import Link from 'next/link'
import { LandingHeader } from '@/components/landing/LandingHeader'
import { LandingIntro } from '@/components/landing/LandingIntro'
import { LeeHero } from '@/components/landing/LeeHero'
import { DisciplineRail, type RailDiscipline } from '@/components/landing/DisciplineRail'
import { Manifesto } from '@/components/landing/Manifesto'
import { PaymentStory } from '@/components/landing/PaymentStory'
import type { StoryPaper } from '@/components/landing/StoryWindow'
import { LandingFooter } from '@/components/landing/LandingFooter'
import { LiveNumbers } from '@/components/landing/LiveNumbers'
import { Reveal } from '@/components/landing/Reveal'
import { CATALOG, DISCIPLINE_COUNT, PAPER_COUNT } from '@/lib/catalog'
import { COPY } from '@/lib/copy'

const L = COPY.landing
const LEE = L.lee

// Disciplinas del carrusel: conteo y el título más reciente de cada una, de mayor a menor.
const RAIL_DISCIPLINES: RailDiscipline[] = [...new Set(CATALOG.map((p) => p.discipline).filter((d): d is string => Boolean(d)))]
  .map((name) => {
    const inDiscipline = CATALOG.filter((p) => p.discipline === name).sort((a, b) => b.publishedDate.localeCompare(a.publishedDate))
    return { name, count: inDiscipline.length, sample: inDiscipline[0].title }
  })
  .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name, 'es'))

// El artículo que recorre la historia del pago. Solo se mandan al cliente los textos que usa.
const STORY_SOURCE = CATALOG.find((p) => p.id === 'autonomous-ai-micropayments') ?? CATALOG[0]
const STORY_PAPER: StoryPaper = {
  discipline: STORY_SOURCE.discipline ?? '',
  title: STORY_SOURCE.title,
  authors: STORY_SOURCE.authors.join(', '),
  abstract: STORY_SOURCE.abstract,
  body: STORY_SOURCE.fullContentMarkdown
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#'))
    .slice(0, 3),
}

export const metadata: Metadata = {
  title: LEE.metaTitle,
  description: LEE.metaDescription,
  openGraph: { title: LEE.metaTitle, description: LEE.metaDescription },
  twitter: { title: LEE.metaTitle, description: LEE.metaDescription },
}

// Landing A ("la historia del pago") para campañas: intro de carga, hero fijo con el símbolo 3D, manifiesto y la
// historia del pago contada con scroll.
export default function LeeLandingPage() {
  return (
    <>
      <LandingIntro />
      <LandingHeader />
      <main>
        <LeeHero />

        <section className="px-4 py-28 sm:px-6 sm:py-40">
          <div className="mx-auto max-w-5xl">
            <Manifesto segments={LEE.manifesto} />
          </div>
        </section>

        <PaymentStory eyebrow={LEE.story.eyebrow} title={LEE.story.title} steps={LEE.story.steps} paper={STORY_PAPER} />

        <DisciplineRail eyebrow={LEE.rail.eyebrow} title={LEE.rail.title} subtitle={LEE.rail.subtitle} disciplines={RAIL_DISCIPLINES} />

        <LiveNumbers paperCount={PAPER_COUNT} disciplineCount={DISCIPLINE_COUNT} />

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
      <LandingFooter />
    </>
  )
}
