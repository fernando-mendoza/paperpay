// Escena three.js de la lluvia de monedas (paso "Stellar lo liquida"). Se importa con import() desde
// CoinRain: vive en su propio chunk, como la del símbolo. Las posiciones salen de coin-rain.ts y del
// progreso del paso, no del reloj: lo que se ve depende solo del scroll (y las capturas son estables).

import {
  AmbientLight,
  CanvasTexture,
  CylinderGeometry,
  DirectionalLight,
  InstancedMesh,
  MeshStandardMaterial,
  NeutralToneMapping,
  Object3D,
  OrthographicCamera,
  PMREMGenerator,
  Scene,
  SRGBColorSpace,
  WebGLRenderer,
} from 'three'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'
import { COIN_COUNT, destinationOf, flightOf, jitter } from './coin-rain'

interface Options {
  box: HTMLElement
  /** Progreso del paso de liquidación, 0→1. */
  progress: () => number
  /** El paso de liquidación está en pantalla. */
  active: () => boolean
  /** Las cajas a donde caen las monedas (editorial y tesorería). */
  targets: () => { publisher: HTMLElement | null; treasury: HTMLElement | null }
}

const USDC_BLUE = '#2775CA'

const smooth = (a: number, b: number, v: number) => {
  const t = Math.min(1, Math.max(0, (v - a) / (b - a)))
  return t * t * (3 - 2 * t)
}

/** La cara de la moneda: azul USDC, un anillo y el signo de pesos/dólar en blanco. */
function faceTexture(): CanvasTexture {
  const size = 128
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const ctx = canvas.getContext('2d')
  if (ctx) {
    ctx.fillStyle = USDC_BLUE
    ctx.fillRect(0, 0, size, size)
    ctx.strokeStyle = 'rgba(255,255,255,0.55)'
    ctx.lineWidth = 6
    ctx.beginPath()
    ctx.arc(size / 2, size / 2, size * 0.36, 0, Math.PI * 2)
    ctx.stroke()
    ctx.fillStyle = '#FFFFFF'
    ctx.font = `700 ${size * 0.5}px system-ui, -apple-system, "Segoe UI", sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText('$', size / 2, size / 2 + size * 0.03)
  }
  const texture = new CanvasTexture(canvas)
  texture.colorSpace = SRGBColorSpace
  return texture
}

/** Monta el canvas dentro de `box`. Devuelve null si no hay WebGL (la historia sigue sin monedas). */
export function createCoinScene({ box, progress, active, targets }: Options): { dispose: () => void } | null {
  let renderer: WebGLRenderer
  try {
    renderer = new WebGLRenderer({ antialias: true, alpha: true, powerPreference: 'low-power' })
  } catch {
    return null
  }
  const mobile = window.matchMedia('(max-width: 767px)').matches
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, mobile ? 1.5 : 2))
  renderer.toneMapping = NeutralToneMapping
  renderer.domElement.setAttribute('aria-hidden', 'true')
  renderer.domElement.className = 'absolute inset-0 h-full w-full'
  box.appendChild(renderer.domElement)

  const scene = new Scene()
  const pmrem = new PMREMGenerator(renderer)
  const envTexture = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
  scene.environment = envTexture
  scene.environmentIntensity = 0.9
  const key = new DirectionalLight('#ffffff', 1.4)
  key.position.set(-120, 200, 300)
  scene.add(key, new AmbientLight('#ffffff', 0.35))

  // Cámara ortográfica en pixeles CSS: el punto (x, y) de la caja es (x, -y) en la escena.
  const camera = new OrthographicCamera(0, 1, 0, -1, -1000, 1000)

  const texture = faceTexture()
  const rim = new MeshStandardMaterial({ color: '#1C5DA6', metalness: 0.65, roughness: 0.32 })
  const face = new MeshStandardMaterial({ map: texture, metalness: 0.25, roughness: 0.38 })
  let geometry = new CylinderGeometry(1, 1, 0.16, 40)
  // Grupos del cilindro: canto, tapa de arriba y tapa de abajo.
  const coins = new InstancedMesh(geometry, [rim, face, face], COIN_COUNT)
  coins.frustumCulled = false
  scene.add(coins)
  const dummy = new Object3D()

  let width = 0
  let height = 0
  let radius = 9
  // Último progreso dibujado (-1: hay que redibujar) y si quedaron monedas en el canvas.
  let lastProgress = -1
  let shown = false
  const resize = () => {
    width = box.clientWidth
    height = box.clientHeight
    if (!width || !height) return
    renderer.setSize(width, height, false)
    camera.right = width
    camera.bottom = -height
    camera.updateProjectionMatrix()
    // Monedas de ~22 px en escritorio y ~16 px en el celular.
    radius = width < 420 ? 8 : 11
    geometry.dispose()
    geometry = new CylinderGeometry(radius, radius, radius * 0.18, 40)
    coins.geometry = geometry
    lastProgress = -1
  }

  // Centro de una caja de destino, en pixeles de `box`. `spread` reparte las monedas a lo ancho.
  const boxRect = { left: 0, top: 0 }
  const targetPoint = (el: HTMLElement | null, spread: number) => {
    if (!el) return { x: width / 2, y: height - 30 }
    const r = el.getBoundingClientRect()
    return {
      x: r.left - boxRect.left + r.width * (0.2 + 0.6 * spread),
      y: r.top - boxRect.top + r.height * 0.55,
    }
  }

  const draw = () => {
    const on = active()
    const p = on ? progress() : 0
    if (!on) {
      if (shown) {
        coins.visible = false
        renderer.render(scene, camera)
        shown = false
      }
      lastProgress = -1
      return
    }
    if (p === lastProgress && shown) return
    lastProgress = p
    shown = true
    coins.visible = true

    const b = box.getBoundingClientRect()
    boxRect.left = b.left
    boxRect.top = b.top
    const { publisher, treasury } = targets()

    for (let i = 0; i < COIN_COUNT; i++) {
      const f = flightOf(i, p)
      if (f <= 0 || f >= 1) {
        dummy.scale.setScalar(0)
        dummy.position.set(0, 0, 0)
        dummy.updateMatrix()
        coins.setMatrixAt(i, dummy.matrix)
        continue
      }
      const to = targetPoint(destinationOf(i) === 'treasury' ? treasury : publisher, jitter(i, 2))
      const sx = width * (0.32 + 0.36 * jitter(i, 1))
      const sy = -radius * 3
      // El chorro baja recto y se abre hacia su caja en la segunda mitad: así se ve partirse.
      const cx = sx + (to.x - sx) * 0.1
      const cy = height * (0.32 + 0.12 * jitter(i, 3))
      const u = 1 - f
      const x = u * u * sx + 2 * u * f * cx + f * f * to.x
      const y = u * u * sy + 2 * u * f * cy + f * f * to.y

      dummy.position.set(x, -y, 40 + i * 0.5)
      // Giran mientras caen (según el progreso, no el reloj) y se achican al entrar en su caja.
      dummy.rotation.set(
        Math.PI / 2 + (jitter(i, 4) - 0.5) * 1.1 + f * (1.5 + 2.5 * jitter(i, 5)) * Math.PI,
        (jitter(i, 6) - 0.5) * 0.9,
        f * (jitter(i, 7) - 0.5) * 3,
      )
      dummy.scale.setScalar(1 - 0.5 * smooth(0.78, 1, f))
      dummy.updateMatrix()
      coins.setMatrixAt(i, dummy.matrix)
    }
    coins.instanceMatrix.needsUpdate = true
    renderer.render(scene, camera)
  }

  let frame = 0
  let visible = true
  const loop = () => {
    draw()
    frame = requestAnimationFrame(loop)
  }
  const start = () => {
    if (frame || !visible || document.hidden) return
    frame = requestAnimationFrame(loop)
  }
  const stop = () => {
    cancelAnimationFrame(frame)
    frame = 0
  }
  // Solo se dibuja con la ventana en pantalla y la pestaña visible.
  const observer = new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting
    if (visible) start()
    else stop()
  })
  observer.observe(box)
  const onVisibility = () => (document.hidden ? stop() : start())
  document.addEventListener('visibilitychange', onVisibility)
  const sizer = new ResizeObserver(() => {
    resize()
    draw()
  })
  sizer.observe(box)

  resize()
  draw()
  start()

  return {
    dispose: () => {
      stop()
      observer.disconnect()
      sizer.disconnect()
      document.removeEventListener('visibilitychange', onVisibility)
      geometry.dispose()
      rim.dispose()
      face.dispose()
      texture.dispose()
      coins.dispose()
      envTexture.dispose()
      pmrem.dispose()
      renderer.dispose()
      renderer.forceContextLoss()
      renderer.domElement.remove()
    },
  }
}
