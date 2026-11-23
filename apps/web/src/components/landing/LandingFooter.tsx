import { COPY } from '@/lib/copy'

export function LandingFooter() {
  return (
    <footer className="border-t border-border px-4 py-8 sm:px-6">
      <div className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-4 text-xs text-muted-foreground">
        <p>{COPY.landing.footer}</p>
        <p>© 2026 {COPY.brand.name}</p>
      </div>
    </footer>
  )
}
