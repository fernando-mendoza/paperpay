import type { Metadata } from 'next'
import { GiftClaimView } from '@/views/GiftClaimView'

// El enlace lleva la llave del regalo en el fragmento (#…): nunca llega al servidor ni a buscadores.
export const metadata: Metadata = {
  title: 'Te regalaron un artículo',
  robots: { index: false, follow: false },
  referrer: 'no-referrer',
}

export default function GiftPage() {
  return <GiftClaimView />
}
