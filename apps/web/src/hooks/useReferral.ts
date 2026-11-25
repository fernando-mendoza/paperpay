'use client'

import { useEffect, useState } from 'react'
import { readReferral, REFERRAL_EVENT } from '@/lib/referral'
import { getPaymentTerms } from '@/services/apiBackend'
import { isFreighter } from '@/services/wallet'

/**
 * ¿Esta API paga referidos (`extra.splitter.referrals` en el 402) con la wallet que paga por el
 * contrato (Freighter)? Y, si sí, el referido guardado en este navegador.
 */
export function useReferral(paperId: string): { enabled: boolean; referrer?: string } {
  const [enabled, setEnabled] = useState(false)
  const [referrer, setReferrer] = useState<string | undefined>()

  useEffect(() => {
    let cancelled = false
    if (!isFreighter()) return
    getPaymentTerms(paperId)
      .then((terms) => !cancelled && setEnabled(terms.extra?.splitter?.referrals === true))
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [paperId])

  useEffect(() => {
    const refresh = () => setReferrer(readReferral())
    refresh()
    window.addEventListener(REFERRAL_EVENT, refresh)
    return () => window.removeEventListener(REFERRAL_EVENT, refresh)
  }, [])

  return { enabled, referrer: enabled ? referrer : undefined }
}
