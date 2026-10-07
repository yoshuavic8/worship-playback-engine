import type { SVGProps } from "react";

export function Metronome(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M9.2 3.6 5 20h14L14.8 3.6a1 1 0 0 0-1-.6h-3.6a1 1 0 0 0-1 .6Z" />
      <path d="M7 15h10" />
      <path d="m12 15 6-9" />
    </svg>
  );
}
