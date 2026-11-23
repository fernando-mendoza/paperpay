// Geometría del símbolo de PaperPay en 3D, a partir de los mismos trazos que LogoMark (viewBox 24×24).
// Cada "P" es un trazo abierto: palo vertical, tramo horizontal arriba, medio círculo a la derecha y
// tramo horizontal de vuelta al palo. Aquí se convierte en el contorno del trazo (ancho fijo) y se extruye.
// Solo lo importa logo3d-scene.ts, que se carga aparte: three nunca entra al bundle inicial.

import { ExtrudeGeometry, Shape, Vector2 } from 'three'

/** Parámetros de cada "P" en coordenadas del SVG (y hacia abajo). */
const LAYERS = [
  { stem: 3, top: 1.8, radius: 7.8 },
  { stem: 6.2, top: 5, radius: 4.6 },
  { stem: 9.4, top: 8.2, radius: 1.4 },
] as const

const BOWL_X = 12.4
const STEM_BOTTOM = 22
const HALF_WIDTH = 0.85 // trazo de 2 en el SVG; el bisel suma el resto
const CENTER = { x: 11.6, y: 11.4 } // centro aproximado del símbolo, para girar sobre sí mismo

type Point = [number, number]

function strokePath(layer: (typeof LAYERS)[number]): Point[] {
  const { stem, top, radius } = layer
  const cy = top + radius
  const points: Point[] = [
    [stem, STEM_BOTTOM],
    [stem, top],
    [BOWL_X, top],
  ]
  const segments = Math.max(14, Math.round(radius * 7))
  for (let i = 1; i <= segments; i++) {
    const angle = -Math.PI / 2 + (Math.PI * i) / segments
    points.push([BOWL_X + radius * Math.cos(angle), cy + radius * Math.sin(angle)])
  }
  // El tramo de vuelta termina en el borde del palo, no en su eje: así el contorno no se cruza consigo mismo.
  points.push([stem + HALF_WIDTH, cy + radius])
  return points
}

// Contorno del trazo: desplaza la polilínea a cada lado con unión en inglete.
function outline(points: Point[]): Point[] {
  const normals = points.map((_, i) => {
    const prev = points[Math.max(0, i - 1)]
    const next = points[Math.min(points.length - 1, i + 1)]
    const segment = (a: Point, b: Point): Point => {
      const dx = b[0] - a[0]
      const dy = b[1] - a[1]
      const len = Math.hypot(dx, dy) || 1
      return [-dy / len, dx / len]
    }
    const nIn = i > 0 ? segment(prev, points[i]) : segment(points[i], next)
    const nOut = i < points.length - 1 ? segment(points[i], next) : nIn
    let nx = nIn[0] + nOut[0]
    let ny = nIn[1] + nOut[1]
    const len = Math.hypot(nx, ny) || 1
    nx /= len
    ny /= len
    const miter = HALF_WIDTH / Math.max(0.35, nx * nIn[0] + ny * nIn[1])
    return [nx * miter, ny * miter] as Point
  })
  const left = points.map((p, i) => [p[0] + normals[i][0], p[1] + normals[i][1]] as Point)
  const right = points.map((p, i) => [p[0] - normals[i][0], p[1] - normals[i][1]] as Point).reverse()
  return [...left, ...right]
}

/** Tres geometrías extruidas, una por "P", centradas en el símbolo (y hacia arriba). */
export function buildLayerGeometries() {
  return LAYERS.map((layer) => {
    const contour = outline(strokePath(layer))
    const shape = new Shape(contour.map(([x, y]) => new Vector2(x - CENTER.x, -(y - CENTER.y))))
    const geometry = new ExtrudeGeometry(shape, {
      depth: 1.1,
      bevelEnabled: true,
      bevelThickness: 0.22,
      bevelSize: 0.15,
      bevelSegments: 3,
      curveSegments: 1,
    })
    geometry.translate(0, 0, -0.55)
    return geometry
  })
}

/**
 * Dónde se cuelga la etiqueta de cada "P", a alturas distintas para que no se encimen:
 * tinta arriba de su panza, verde al lado derecho de la suya, azul al pie de su palo.
 */
export function labelAnchors(): [number, number][] {
  const [ink, green, blue] = LAYERS
  const diag = Math.SQRT1_2
  const points: Point[] = [
    [BOWL_X + (ink.radius + 1) * diag, ink.top + ink.radius - (ink.radius + 1) * diag],
    [BOWL_X + green.radius + 1, green.top + green.radius],
    [blue.stem + 1, STEM_BOTTOM - 1],
  ]
  return points.map(([x, y]) => [x - CENTER.x, -(y - CENTER.y)])
}
