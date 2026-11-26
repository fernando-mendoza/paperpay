'use client'

import { AnimatePresence, motion, useMotionValueEvent, useTransform, type MotionValue } from 'motion/react'
import { useRef, useState, type RefObject } from 'react'
import { CoinRain } from './CoinRain'
import { landed, usdc } from './coin-rain'

export interface StoryPaper {
  discipline: string
  title: string
  authors: string
  abstract: string
  /** Párrafos del cuerpo que se "desbloquean" en el último paso. */
  body: string[]
}

interface Props {
  /** Paso activo: 0 resumen · 1 firma · 2 liquidación · 3 lectura. */
  active: number
  /** Progreso 0→1 de la sección completa (sticky). */
  progress: MotionValue<number>
  steps: number
  paper: StoryPaper
}

const EASE = [0.22, 1, 0.36, 1] as const
// El contrato de reparto que usa el sitio (v0.4.0): Freighter muestra la llamada a `pay` sobre él.
const SPLITTER_SHORT = 'CC5S…ADGA'
// Un pago real de $0.50 firmado en Freighter: 0.49 a la editorial y 0.01 a la tesorería.
const REAL_TX = '3d7b2c5625e5869a174925aa8ca1d221c5edb9e54843f6638eaf25060b9c88f4'

// Lo que se ilumina en cada paso. Lo demás se atenúa para que el ojo vaya a un solo lugar.
function spot(on: boolean, dimmed: boolean) {
  if (on) return 'relative z-10 rounded-xl ring-2 ring-primary/70 shadow-[0_0_0_6px_rgba(47,91,255,0.12),0_16px_40px_rgba(47,91,255,0.22)]'
  return dimmed ? 'opacity-30' : ''
}

// Ventana de navegador con un artículo de PaperPay, hecha en HTML. Cambia con cada paso de la historia.
export function StoryWindow({ active, progress, steps, paper }: Props) {
  const unlocked = active === 3
  // Progreso dentro del paso de liquidación (índice 2): mueve el cronómetro, las monedas y el reparto.
  const settle = useTransform(progress, (v) => Math.min(1, Math.max(0, v * steps - 2)))
  const publisherRef = useRef<HTMLDivElement>(null)
  const treasuryRef = useRef<HTMLDivElement>(null)

  return (
    <div className="relative overflow-hidden rounded-2xl border border-white/10 bg-card text-foreground shadow-[0_40px_100px_rgba(0,0,0,0.5)]">
      <div className="flex items-center gap-2 border-b border-border bg-[#F4F1EA] px-3 py-2.5 sm:px-4">
        <span className="h-2.5 w-2.5 rounded-full bg-[#E2DDD2]" />
        <span className="h-2.5 w-2.5 rounded-full bg-[#E2DDD2]" />
        <span className="h-2.5 w-2.5 rounded-full bg-[#E2DDD2]" />
        <span className="ml-2 min-w-0 flex-1 truncate rounded-md bg-white px-3 py-1 text-[11px] text-muted-foreground">
          paperpay.blockchains.tf/papers/autonomous-ai-micropayments
        </span>
        <span
          className={`flex h-6 w-6 flex-none items-center justify-center rounded-md transition-colors duration-500 ${
            active === 1 ? 'bg-primary text-white' : 'bg-white text-muted-foreground'
          }`}
          aria-hidden
        >
          <WalletIcon />
        </span>
      </div>

      <div className="relative h-[clamp(300px,47svh,470px)] overflow-hidden px-5 pt-5 sm:px-7 sm:pt-6">
        <div className={`transition-opacity duration-500 ${active === 0 || unlocked ? '' : 'opacity-30'}`}>
          <div className="mb-3 flex items-center gap-2">
            <span className="rounded-full bg-[#EEF2FF] px-2.5 py-1 text-[11px] font-semibold text-[#4338CA]">{paper.discipline}</span>
            <span className="rounded-full bg-usdc-bg px-2.5 py-1 text-[11px] font-semibold text-usdc">0.50 USDC</span>
          </div>
          <h3 className="mb-1 font-display text-lg leading-snug font-semibold sm:text-xl">{paper.title}</h3>
          <p className="mb-4 text-xs text-muted-foreground">{paper.authors}</p>
        </div>

        <div className={`p-3 transition-all duration-500 ${spot(active === 0, active === 1 || active === 2)}`}>
          <p className="mb-1 text-[11px] font-semibold tracking-wider text-primary uppercase">Resumen · gratis</p>
          <p className="line-clamp-3 text-[13px] leading-relaxed text-foreground/80">{paper.abstract}</p>
        </div>

        <div className="relative mt-3">
          {unlocked ? (
            <motion.div
              initial={{ opacity: 0, filter: 'blur(10px)' }}
              animate={{ opacity: 1, filter: 'blur(0px)' }}
              transition={{ duration: 0.9, ease: EASE }}
              className="space-y-2 px-3"
            >
              {paper.body.map((paragraph) => (
                <p key={paragraph} className="text-[13px] leading-relaxed text-foreground/85">
                  {paragraph}
                </p>
              ))}
            </motion.div>
          ) : (
            <div className="space-y-2.5 px-3 blur-[3px]" aria-hidden>
              {[100, 92, 97, 84, 95, 88].map((w, i) => (
                <span key={i} className="block h-2.5 rounded bg-[#ECEAE2]" style={{ width: `${w}%` }} />
              ))}
            </div>
          )}

          <AnimatePresence>
            {active <= 1 && (
              <motion.div
                key="paywall"
                exit={{ opacity: 0, y: 8 }}
                transition={{ duration: 0.3 }}
                className="absolute inset-x-0 top-6 flex justify-center"
              >
                <span
                  className={`inline-flex items-center gap-2 rounded-xl bg-primary px-4 py-2.5 text-xs font-semibold text-primary-foreground transition-all duration-500 ${
                    active === 0 ? 'opacity-40' : ''
                  }`}
                >
                  <LockIcon /> Leer por $0.50 USDC
                </span>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        <AnimatePresence>
          {active === 1 && <FreighterPopup key="freighter" />}
          {active === 2 && (
            <SettlementCard key="settle" local={settle} publisherRef={publisherRef} treasuryRef={treasuryRef} />
          )}
          {unlocked && <AccessBadge key="access" />}
        </AnimatePresence>
        <CoinRain progress={settle} active={active === 2} publisherRef={publisherRef} treasuryRef={treasuryRef} />
      </div>
    </div>
  )
}

function FreighterPopup() {
  return (
    <motion.div
      initial={{ opacity: 0, y: -14, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -10, scale: 0.98 }}
      transition={{ duration: 0.45, ease: EASE }}
      className="absolute top-2 right-2 z-20 w-[min(270px,82%)] rounded-xl border border-border bg-white p-4 shadow-[0_18px_50px_rgba(15,23,42,0.25)]"
    >
      <div className="mb-3 flex items-center justify-between">
        <span className="flex items-center gap-2 text-sm font-semibold">
          <span className="flex h-6 w-6 items-center justify-center rounded-md bg-primary text-white">
            <WalletIcon />
          </span>
          Freighter
        </span>
        <span className="rounded-full bg-warning/15 px-2 py-0.5 text-[10px] font-semibold text-[#B45309]">Testnet</span>
      </div>
      <p className="text-[11px] text-muted-foreground">Solicitud de firma de</p>
      <p className="mb-3 text-xs font-semibold">paperpay.blockchains.tf</p>
      <dl className="mb-4 space-y-1.5 rounded-lg bg-muted px-3 py-2.5 text-[11px]">
        <Row label="Pago" value="0.50 USDC" strong />
        <Row label="Contrato" value={`${SPLITTER_SHORT} · pay`} mono />
        <Row label="Comisión de red" value="< $0.01" />
      </dl>
      <div className="grid grid-cols-2 gap-2">
        <span className="rounded-lg border border-border py-2 text-center text-xs font-semibold text-muted-foreground">Rechazar</span>
        <span className="rounded-lg bg-primary py-2 text-center text-xs font-semibold text-white ring-2 ring-primary/40 ring-offset-2 shadow-[0_0_24px_rgba(47,91,255,0.55)]">
          Firmar
        </span>
      </div>
    </motion.div>
  )
}

function SettlementCard({
  local,
  publisherRef,
  treasuryRef,
}: {
  local: MotionValue<number>
  publisherRef: RefObject<HTMLDivElement | null>
  treasuryRef: RefObject<HTMLDivElement | null>
}) {
  // El cronómetro llega a ~5 s y se confirma en 0.70, cuando cae la última moneda.
  const seconds = useTransform(local, (v) => `${(Math.min(v / 0.7, 1) * 4.8).toFixed(1)} s`)
  const [confirmed, setConfirmed] = useState(() => local.get() >= 0.7)
  // Cada moneda es un centavo: los contadores suben a medida que llegan (49 → 0.49, 1 → 0.01).
  const [cents, setCents] = useState(() => landed(local.get()))
  useMotionValueEvent(local, 'change', (v) => {
    setConfirmed(v >= 0.7)
    const next = landed(v)
    setCents((current) => (current.publisher === next.publisher && current.treasury === next.treasury ? current : next))
  })

  return (
    <motion.div
      initial={{ opacity: 0, y: 24 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 12 }}
      transition={{ duration: 0.45, ease: EASE }}
      className="absolute inset-x-4 bottom-4 z-20 rounded-xl border border-border bg-white p-4 ring-2 ring-primary/60 shadow-[0_0_0_6px_rgba(47,91,255,0.1),0_18px_50px_rgba(15,23,42,0.22)] sm:inset-x-7"
    >
      <div className="mb-3 flex items-center justify-between">
        <span className="text-xs font-semibold">Liquidando en Stellar</span>
        <motion.span className="font-mono text-sm font-semibold text-primary tabular-nums">{seconds}</motion.span>
      </div>
      <ol className="space-y-1.5 text-[12px]">
        <Check done label="Pago firmado en Freighter" />
        <Check done label="Transacción enviada a la red" />
        <Check done={confirmed} label={confirmed ? 'Confirmada en el ledger' : 'Esperando confirmación…'} />
      </ol>
      <div className="mt-3 grid grid-cols-[1fr_auto] gap-2">
        <div ref={publisherRef} data-split="editorial" className="rounded-lg bg-success-bg px-3 py-1.5">
          <p className="text-[10px] font-semibold tracking-wider text-success-strong/80 uppercase">Editorial · 98%</p>
          <p className="font-mono text-sm font-semibold text-success-strong tabular-nums">{usdc(cents.publisher)} USDC</p>
        </div>
        <div ref={treasuryRef} data-split="paperpay" className="rounded-lg bg-muted px-3 py-1.5">
          <p className="text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">PaperPay · 2%</p>
          <p className="font-mono text-sm font-semibold text-foreground tabular-nums">{usdc(cents.treasury)} USDC</p>
        </div>
      </div>
      <a
        href={`https://stellar.expert/explorer/testnet/tx/${REAL_TX}`}
        target="_blank"
        rel="noreferrer"
        className="mt-2 block truncate rounded-md bg-muted px-2.5 py-1.5 font-mono text-[10px] text-muted-foreground hover:text-foreground"
      >
        tx {REAL_TX.slice(0, 6)}…{REAL_TX.slice(-4)} · un pago real, en stellar.expert
      </a>
    </motion.div>
  )
}

function AccessBadge() {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.9, y: 10 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.5, ease: EASE, delay: 0.25 }}
      className="absolute top-4 right-4 z-20 inline-flex items-center gap-2 rounded-full bg-success-bg px-3 py-1.5 text-xs font-semibold text-success-strong ring-2 ring-success/50 shadow-[0_0_0_6px_rgba(16,185,129,0.12)]"
    >
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
        <path d="M3 6l2 2 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      Acceso por 24 h
    </motion.div>
  )
}

function Row({ label, value, strong, mono }: { label: string; value: string; strong?: boolean; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={`${strong ? 'font-semibold text-foreground' : 'text-foreground'} ${mono ? 'font-mono' : ''}`}>{value}</dd>
    </div>
  )
}

function Check({ done, label }: { done: boolean; label: string }) {
  return (
    <li className="flex items-center gap-2">
      <span
        className={`flex h-4 w-4 flex-none items-center justify-center rounded-full transition-colors duration-300 ${
          done ? 'bg-success text-white' : 'border border-border-strong'
        }`}
      >
        {done && (
          <svg width="9" height="9" viewBox="0 0 12 12" fill="none" aria-hidden>
            <path d="M3 6l2 2 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        )}
      </span>
      <span className={done ? 'text-foreground' : 'text-muted-foreground'}>{label}</span>
    </li>
  )
}

function WalletIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden>
      <rect x="1.75" y="3.5" width="12.5" height="9.5" rx="2" stroke="currentColor" strokeWidth="1.4" />
      <path d="M10.5 8.25h3.75" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      <path d="M3.5 3.5l6.5-2v2" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
    </svg>
  )
}

function LockIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden>
      <rect x="2.5" y="7" width="11" height="7.5" rx="1.5" stroke="currentColor" strokeWidth="1.4" />
      <path d="M5.5 7V5.5a2.5 2.5 0 015 0V7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  )
}
