/*
 * Lectura mínima de los parámetros que Horizon devuelve en una operación invoke_host_function
 * (cada uno es un ScVal en XDR base64). Basta con leer unos bytes del encabezado; así el panel y el
 * contador de la landing no cargan el SDK de Stellar (~1 MB) solo para contar lecturas.
 *
 * ScVal en XDR: discriminante int32, luego el valor.
 *   SCV_SYMBOL (15)  → uint32 largo + bytes
 *   SCV_VEC (16)     → uint32 presente (1) + uint32 largo + elementos
 *   SCV_ADDRESS (18) → int32 tipo (1 = contrato) + 32 bytes del id
 */

const SCV_SYMBOL = 15
const SCV_VEC = 16
const SCV_ADDRESS = 18
const SC_ADDRESS_CONTRACT = 1

const BASE32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'

function base64ToBytes(value: string): Uint8Array {
  return Uint8Array.from(atob(value), (c) => c.charCodeAt(0))
}

function bytesToBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
}

function u32(bytes: Uint8Array, offset: number): number {
  if (bytes.length < offset + 4) return -1
  return new DataView(bytes.buffer, bytes.byteOffset).getUint32(offset)
}

/** Los 32 bytes del id de un contrato "C…" (strkey: versión + 32 bytes + CRC, en base32). */
function contractIdBytes(contractId: string): Uint8Array | null {
  if (!/^C[A-Z2-7]{55}$/.test(contractId)) return null
  const out: number[] = []
  let value = 0
  let bits = 0
  for (const ch of contractId) {
    value = ((value << 5) | BASE32.indexOf(ch)) & 0xffff
    bits += 5
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 0xff)
      bits -= 8
    }
  }
  return out.length === 35 ? Uint8Array.from(out.slice(1, 33)) : null
}

/** El ScVal (base64) con que Horizon muestra la dirección del contrato en `parameters[0]`. */
export function contractAddressParam(contractId: string): string | null {
  const id = contractIdBytes(contractId)
  if (!id) return null
  const bytes = new Uint8Array(40)
  const view = new DataView(bytes.buffer)
  view.setUint32(0, SCV_ADDRESS)
  view.setUint32(4, SC_ADDRESS_CONTRACT)
  bytes.set(id, 8)
  return bytesToBase64(bytes)
}

/** Nombre de la función (ScVal símbolo) o null si el parámetro no es un símbolo. */
export function readScSymbol(value: string | undefined): string | null {
  if (!value) return null
  const bytes = base64ToBytes(value)
  const length = u32(bytes, 4)
  if (u32(bytes, 0) !== SCV_SYMBOL || length < 0 || bytes.length < 8 + length) return null
  return String.fromCharCode(...bytes.subarray(8, 8 + length))
}

/** Cantidad de elementos de un ScVal vector, o 0 si no es un vector. */
export function readScVecLength(value: string | undefined): number {
  if (!value) return 0
  const bytes = base64ToBytes(value)
  if (u32(bytes, 0) !== SCV_VEC || u32(bytes, 4) !== 1) return 0
  return Math.max(0, u32(bytes, 8))
}
