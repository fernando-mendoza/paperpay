// Vacío = mismo dominio: next.config.ts (o la Pages Function en el export estático) reenvía
// /api/* al backend.
export const API_URL = (process.env.NEXT_PUBLIC_API_URL ?? '').replace(/\/$/, '')
