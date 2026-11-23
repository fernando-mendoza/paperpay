import { CATALOG } from '@/lib/catalog'
import { formatLongDate } from '@/lib/format'

// Selección de /top, curada a mano. Para cambiarla, editar esta lista: el id es el del catálogo
// (src/mocks/papers.json) y `why` es la razón de leerlo, sacada del resumen del propio artículo.
// El orden de la lista es el orden en la página.
const PICKS: readonly { id: string; why: string }[] = [
  {
    id: 'crispr-nanomedicine-biotech',
    why: 'Llevar CRISPR a la clínica choca con la entrega. Esta formulación logra 87% de edición dirigida en un modelo murino.',
  },
  {
    id: 'gravitational-lensing-dark-matter',
    why: 'Reconstruye dónde está la materia oscura en 42 cúmulos masivos y lo contrasta con simulaciones cosmológicas.',
  },
  {
    id: 'autonomous-ai-micropayments',
    why: 'Explica el mecanismo con el que se cobra aquí: HTTP 402 y pagos entre agentes liquidados en Stellar.',
  },
  {
    id: 'stablecoins-emerging-markets',
    why: 'Estima cuánto se ahorra en comisiones al enviar remesas con stablecoins frente a los canales tradicionales.',
  },
  {
    id: 'battery-solid-state-electrolyte',
    why: 'Un electrolito sólido con conductividad cercana a la de los líquidos: baterías más seguras y con más energía.',
  },
  {
    id: 'quantum-cryptography-post-quantum',
    why: 'Qué firmas resisten una computadora cuántica (Dilithium, FALCON) y cómo adaptarlas a los libros distribuidos.',
  },
]

/** Lo que la tarjeta de /top necesita de cada artículo. Solo esto viaja al cliente. */
export interface TopPick {
  id: string
  rank: number
  discipline: string
  title: string
  authors: string
  publisher: string
  date: string
  abstract: string
  why: string
}

// Falla al compilar si un id no existe: una tarjeta que desaparece en silencio no se nota en el anuncio.
export function resolveTopPicks(): TopPick[] {
  return PICKS.map(({ id, why }, i) => {
    const paper = CATALOG.find((p) => p.id === id)
    if (!paper) throw new Error(`top-picks: el artículo "${id}" no está en el catálogo`)
    return {
      id,
      rank: i + 1,
      discipline: paper.discipline ?? '',
      title: paper.title,
      authors: paper.authors.join(', '),
      publisher: paper.publisher,
      date: formatLongDate(paper.publishedDate),
      abstract: paper.abstract,
      why,
    }
  })
}
