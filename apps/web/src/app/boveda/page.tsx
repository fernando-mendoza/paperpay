import type { Metadata } from 'next'
import { COPY } from '@/lib/copy'
import { BovedaView } from '@/views/BovedaView'

// Pantalla del reto exprés de Ship It: no es parte del catálogo, así que no se indexa.
export const metadata: Metadata = {
  title: COPY.boveda.metaTitle,
  robots: { index: false, follow: false },
}

export default function BovedaPage() {
  return <BovedaView />
}
