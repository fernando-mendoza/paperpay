import { LogoMark } from '@/components/LogoMark'
import { COPY } from '@/lib/copy'

// Intro de /lee: el símbolo, las letras de la marca subiendo desde una máscara y una raya de carga;
// a los ~0.85 s la capa sube como cortina y entra el hero (sus palabras suben con `.lee-word`).
// Es solo CSS (globals.css, "Intro de /lee"): no espera a que React hidrate y funciona sin JS.
// Con reduced-motion no se muestra. Es decorativa: el contenido real está debajo desde el primer cuadro.
export function LandingIntro() {
  const letters = [...COPY.brand.name]
  return (
    <div
      className="lee-intro pointer-events-none fixed inset-0 z-50 flex flex-col items-center justify-center gap-5 bg-background"
      aria-hidden
    >
      <LogoMark size={56} className="lee-intro-mark" />
      <p className="font-display text-2xl leading-none font-semibold text-foreground">
        {letters.map((letter, i) => (
          <span key={i} className="inline-block overflow-hidden pb-[0.12em] align-bottom">
            <span className="lee-intro-letter" style={{ '--i': i } as React.CSSProperties}>
              {letter}
            </span>
          </span>
        ))}
      </p>
      <span className="relative h-0.5 w-28 overflow-hidden rounded-full bg-foreground/10">
        <span className="lee-intro-bar absolute inset-0 rounded-full bg-primary" />
      </span>
    </div>
  )
}
