// Landings para campañas (ads). Viven fuera del catálogo, sin el Navbar del sitio y con noindex.
export const LANDING_ROUTES = ['/lee', '/top'] as const

export function isLandingRoute(pathname: string | null): boolean {
  if (!pathname) return false
  return LANDING_ROUTES.some((route) => pathname === route || pathname.startsWith(`${route}/`))
}
