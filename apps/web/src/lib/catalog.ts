import papers from '@/mocks/papers.json'
import type { PaperFull } from '@/lib/types'

// Catálogo al compilar, para las páginas que lo leen en el servidor (landings, rutas estáticas).
// No importarlo desde componentes cliente: metería los 52 artículos completos en el bundle.
export const CATALOG = papers as PaperFull[]

// Conteos derivados del catálogo: no se escriben a mano para que no se desfasen.
export const PAPER_COUNT = CATALOG.length
export const DISCIPLINE_COUNT = new Set(CATALOG.map((p) => p.discipline).filter(Boolean)).size
