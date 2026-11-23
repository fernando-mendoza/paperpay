// Escena three.js del símbolo de PaperPay. Se importa con import() desde HeroLogo3D: queda en su
// propio chunk y, al usar imports con nombre, el bundler deja fuera lo que three no necesita aquí.

import {
  AmbientLight,
  DirectionalLight,
  Group,
  Mesh,
  MeshPhysicalMaterial,
  NeutralToneMapping,
  PerspectiveCamera,
  PMREMGenerator,
  Scene,
  Vector3,
  WebGLRenderer,
} from 'three'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'
import { buildLayerGeometries, labelAnchors } from './logo3d'

interface Options {
  box: HTMLElement
  /** Color de cada capa (tinta, verde, azul). */
  colors: readonly string[]
  /** Etiqueta HTML de cada capa; se mueve con su ancla proyectada. */
  labels: readonly (HTMLElement | null)[]
  /** 0→1: reposo → capas separadas → se juntan con un giro. */
  progress: () => number
  /** Un cuadro fijo, sin bucle ni puntero. */
  still: boolean
}

const SPREAD = 5.5 // separación en z entre capas cuando el símbolo se abre
const DISTANCE = 54

const smooth = (a: number, b: number, v: number) => {
  const t = Math.min(1, Math.max(0, (v - a) / (b - a)))
  return t * t * (3 - 2 * t)
}

/** Monta el canvas dentro de `box`. Devuelve null si no hay WebGL (se queda el SVG). */
export function createLogoScene({ box, colors, labels, progress, still }: Options): { dispose: () => void } | null {
  let renderer: WebGLRenderer
  try {
    renderer = new WebGLRenderer({ antialias: true, alpha: true, powerPreference: 'low-power' })
  } catch {
    return null
  }
  const mobile = window.matchMedia('(max-width: 767px)').matches
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, mobile ? 1.5 : 2))
  // Neutral conserva los colores de la marca; ACES los lava (el verde salía menta).
  renderer.toneMapping = NeutralToneMapping
  renderer.domElement.setAttribute('aria-hidden', 'true')
  renderer.domElement.className = 'absolute inset-0 h-full w-full'
  box.appendChild(renderer.domElement)

  const scene = new Scene()
  const pmrem = new PMREMGenerator(renderer)
  const envTexture = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
  scene.environment = envTexture
  scene.environmentIntensity = 0.85

  const key = new DirectionalLight('#ffffff', 1.6)
  key.position.set(-14, 20, 26)
  scene.add(key, new AmbientLight('#ffffff', 0.25))

  const camera = new PerspectiveCamera(32, 1, 1, 200)
  camera.position.set(0, 0, DISTANCE)

  const geometries = buildLayerGeometries()
  const materials = colors.map(
    (color, i) =>
      new MeshPhysicalMaterial({ color, roughness: i === 0 ? 0.42 : 0.3, metalness: 0, clearcoat: 0.7, clearcoatRoughness: 0.25 }),
  )
  const group = new Group()
  const meshes = geometries.map((geometry, i) => {
    const mesh = new Mesh(geometry, materials[i])
    // Juntas, las capas quedan escalonadas un poco en z, como en el SVG (la azul encima).
    mesh.position.z = i * 0.35
    group.add(mesh)
    return mesh
  })
  scene.add(group)

  const anchors = labelAnchors().map(([x, y]) => new Vector3(x, y, 0.8))
  const projected = new Vector3()

  let width = 0
  let height = 0
  const resize = () => {
    width = box.clientWidth
    height = box.clientHeight
    if (!width || !height) return
    renderer.setSize(width, height, false)
    camera.aspect = width / height
    // En cajas angostas se aleja la cámara para que el símbolo abierto no se corte de lado.
    camera.position.z = DISTANCE / Math.min(1, Math.max(0.6, camera.aspect))
    camera.updateProjectionMatrix()
  }
  resize()

  const pointer = { x: 0, y: 0, tx: 0, ty: 0 }
  const onPointer = (e: PointerEvent) => {
    if (e.pointerType !== 'mouse') return
    pointer.tx = (e.clientX / window.innerWidth) * 2 - 1
    pointer.ty = (e.clientY / window.innerHeight) * 2 - 1
  }
  if (!still) window.addEventListener('pointermove', onPointer, { passive: true })

  const startedAt = performance.now()
  const draw = () => {
    const p = still ? 0 : progress()
    const t = still ? 0 : (performance.now() - startedAt) / 1000
    pointer.x += (pointer.tx - pointer.x) * 0.06
    pointer.y += (pointer.ty - pointer.y) * 0.06

    const open = smooth(0.1, 0.42, p) * (1 - smooth(0.6, 0.88, p))
    const spin = smooth(0.6, 0.95, p)
    group.rotation.y = -0.36 - 0.6 * open + spin * Math.PI * 2 + pointer.x * 0.28 + Math.sin(t * 0.6) * 0.05
    group.rotation.x = 0.1 + 0.22 * open + pointer.y * 0.16 + Math.sin(t * 0.45) * 0.025
    group.position.y = Math.sin(t * 0.8) * 0.25 - open * 0.6
    meshes.forEach((mesh, i) => {
      mesh.position.z = i * 0.35 + (i - 1) * SPREAD * open
    })
    renderer.render(scene, camera)

    // Etiquetas HTML pegadas a cada capa: se proyecta el ancla a la pantalla.
    meshes.forEach((mesh, i) => {
      const el = labels[i]
      if (!el) return
      projected.copy(anchors[i])
      mesh.localToWorld(projected)
      projected.project(camera)
      const x = ((projected.x + 1) / 2) * width
      const y = ((1 - projected.y) / 2) * height
      const left = Math.min(Math.max(8, x + 10), width - el.offsetWidth - 8)
      el.style.transform = `translate3d(${left}px, ${y - el.offsetHeight / 2}px, 0)`
      el.style.opacity = String(Math.min(1, Math.max(0, (open - 0.5) / 0.4)))
    })
  }

  let frame = 0
  let visible = true
  const loop = () => {
    draw()
    frame = requestAnimationFrame(loop)
  }
  const start = () => {
    if (still || frame || !visible || document.hidden) return
    frame = requestAnimationFrame(loop)
  }
  const stop = () => {
    cancelAnimationFrame(frame)
    frame = 0
  }

  // Solo se dibuja con el canvas en pantalla y la pestaña visible.
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
    if (still) draw()
  })
  sizer.observe(box)

  draw()
  start()

  return {
    dispose: () => {
      stop()
      observer.disconnect()
      sizer.disconnect()
      document.removeEventListener('visibilitychange', onVisibility)
      window.removeEventListener('pointermove', onPointer)
      geometries.forEach((g) => g.dispose())
      materials.forEach((m) => m.dispose())
      envTexture.dispose()
      pmrem.dispose()
      renderer.dispose()
      renderer.domElement.remove()
    },
  }
}
