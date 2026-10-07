import * as DialogP from "@radix-ui/react-dialog";
import * as SliderP from "@radix-ui/react-slider";
import * as SwitchP from "@radix-ui/react-switch";
import * as TooltipP from "@radix-ui/react-tooltip";
import { Loader2, X } from "lucide-react";
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn, meterLevel } from "@/lib/format";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "outline" | "mon";
type Size = "sm" | "md" | "lg" | "xl";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-accent text-accent-ink hover:bg-accent-2 font-semibold shadow-[0_1px_0_rgb(255_255_255/0.25)_inset]",
  secondary: "bg-raised text-fg hover:bg-[#2b3038] border border-line-2/60",
  ghost: "text-fg-2 hover:text-fg hover:bg-white/5",
  danger: "bg-danger text-white hover:bg-[#f05a5f] font-semibold",
  outline: "border border-line-2 text-fg-2 hover:text-fg hover:border-[#4a505b] hover:bg-white/[0.03]",
  mon: "bg-mon/15 text-mon hover:bg-mon/25 border border-mon/30",
};
const SIZES: Record<Size, string> = {
  sm: "h-7 px-2.5 text-[0.8rem] gap-1.5 rounded-md",
  md: "h-9 px-3.5 text-[0.9rem] gap-2 rounded-lg",
  lg: "h-11 px-5 text-base gap-2 rounded-xl",
  xl: "h-14 px-7 text-lg gap-3 rounded-2xl",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
  icon?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", loading, icon, className, children, disabled, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      className={cn(
        "inline-flex select-none items-center justify-center whitespace-nowrap transition-colors duration-100 disabled:pointer-events-none disabled:opacity-40",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      disabled={disabled || loading}
      {...rest}
    >
      {loading ? <Loader2 className="size-4 animate-spin" /> : icon}
      {children}
    </button>
  );
});

export const IconButton = forwardRef<HTMLButtonElement, ButtonProps & { label: string; active?: boolean }>(
  function IconButton({ label, active, className, size = "md", variant = "ghost", ...rest }, ref) {
    const dims = { sm: "size-7", md: "size-9", lg: "size-11", xl: "size-14" }[size];
    return (
      <Tip label={label}>
        <Button
          ref={ref}
          aria-label={label}
          variant={variant}
          size={size}
          className={cn(dims, "px-0", active && "bg-accent/15 text-accent hover:bg-accent/20 hover:text-accent", className)}
          {...rest}
        />
      </Tip>
    );
  },
);

export function Tip({ label, children, side = "top" }: { label: ReactNode; children: ReactNode; side?: "top" | "bottom" | "left" | "right" }) {
  return (
    <TooltipP.Root delayDuration={350}>
      <TooltipP.Trigger asChild>{children}</TooltipP.Trigger>
      <TooltipP.Portal>
        <TooltipP.Content
          side={side}
          sideOffset={6}
          className="z-50 max-w-xs rounded-md border border-line-2 bg-[#20242a] px-2 py-1 text-xs text-fg-2 shadow-pop"
        >
          {label}
        </TooltipP.Content>
      </TooltipP.Portal>
    </TooltipP.Root>
  );
}

export function Switch({
  checked, onChange, disabled, label, tone = "accent", size = "md",
}: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; label?: string; tone?: "accent" | "mon" | "ok"; size?: "sm" | "md" }) {
  const on = { accent: "data-[state=checked]:bg-accent", mon: "data-[state=checked]:bg-mon", ok: "data-[state=checked]:bg-ok" }[tone];
  return (
    <SwitchP.Root
      checked={checked}
      onCheckedChange={onChange}
      disabled={disabled}
      aria-label={label}
      className={cn(
        "relative shrink-0 rounded-full bg-line-2 transition-colors disabled:opacity-40",
        on,
        size === "md" ? "h-[22px] w-[38px]" : "h-[18px] w-[30px]",
      )}
    >
      <SwitchP.Thumb
        className={cn(
          "block rounded-full bg-white shadow transition-transform",
          size === "md"
            ? "size-[18px] translate-x-[2px] data-[state=checked]:translate-x-[18px]"
            : "size-[14px] translate-x-[2px] data-[state=checked]:translate-x-[14px]",
        )}
      />
    </SwitchP.Root>
  );
}

export function Slider({
  value, onChange, onCommit, min, max, step = 1, disabled, tone = "accent", label, className,
}: {
  value: number; onChange: (v: number) => void; onCommit?: (v: number) => void; min: number; max: number; step?: number;
  disabled?: boolean; tone?: "accent" | "mon" | "fg"; label?: string; className?: string;
}) {
  const color = { accent: "bg-accent", mon: "bg-mon", fg: "bg-fg-2" }[tone];
  return (
    <SliderP.Root
      className={cn("relative flex h-5 w-full touch-none select-none items-center", disabled && "opacity-40", className)}
      value={[value]}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      onValueChange={(v) => onChange(v[0])}
      onValueCommit={(v) => onCommit?.(v[0])}
      aria-label={label}
    >
      <SliderP.Track className="relative h-1.5 grow overflow-hidden rounded-full bg-line-2">
        <SliderP.Range className={cn("absolute h-full", color)} />
      </SliderP.Track>
      <SliderP.Thumb className="block size-4 rounded-full border-2 border-bg bg-white shadow-md outline-none ring-accent/40 focus-visible:ring-4" />
    </SliderP.Root>
  );
}

export function Segmented<T extends string | number>({
  value, onChange, options, size = "md", className, disabled,
}: {
  value: T; onChange: (v: T) => void; options: { value: T; label: ReactNode; title?: string }[];
  size?: "sm" | "md"; className?: string; disabled?: boolean;
}) {
  return (
    <div className={cn("inline-flex rounded-lg border border-line bg-bg-2 p-0.5", disabled && "opacity-40", className)} role="radiogroup">
      {options.map((o) => (
        <button
          key={String(o.value)}
          role="radio"
          aria-checked={o.value === value}
          title={o.title}
          disabled={disabled}
          onClick={() => onChange(o.value)}
          className={cn(
            "rounded-md px-3 font-medium transition-colors",
            size === "md" ? "h-8 text-[0.85rem]" : "h-6 px-2 text-xs",
            o.value === value ? "bg-raised text-fg shadow-sm" : "text-muted hover:text-fg-2",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Badge({ children, tone = "neutral", className, dot }: {
  children: ReactNode; tone?: "neutral" | "ok" | "warn" | "err" | "accent" | "mon" | "live"; className?: string; dot?: boolean;
}) {
  const t = {
    neutral: "bg-white/[0.06] text-fg-2 border-white/[0.06]",
    ok: "bg-ok/12 text-ok border-ok/20",
    warn: "bg-warn/12 text-warn border-warn/25",
    err: "bg-err/12 text-err border-err/25",
    accent: "bg-accent/12 text-accent border-accent/25",
    mon: "bg-mon/12 text-mon border-mon/25",
    live: "bg-live text-white border-live",
  }[tone];
  return (
    <span className={cn("inline-flex h-[1.35rem] items-center gap-1.5 whitespace-nowrap rounded-md border px-1.5 text-[0.72rem] font-semibold", t, className)}>
      {dot && <span className="size-1.5 rounded-full bg-current" />}
      {children}
    </span>
  );
}

export function Dialog({
  open, onOpenChange, title, description, children, footer, width = 520,
}: {
  open: boolean; onOpenChange: (v: boolean) => void; title: ReactNode; description?: ReactNode; children?: ReactNode;
  footer?: ReactNode; width?: number;
}) {
  return (
    <DialogP.Root open={open} onOpenChange={onOpenChange}>
      <DialogP.Portal>
        <DialogP.Overlay className="fixed inset-0 z-40 bg-black/60 backdrop-blur-[2px] data-[state=open]:animate-in" />
        <DialogP.Content
          style={{ width }}
          className="fixed left-1/2 top-1/2 z-50 max-h-[85vh] max-w-[92vw] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-2xl border border-line-2 bg-panel shadow-pop outline-none"
        >
          <div className="flex items-start justify-between gap-4 border-b border-line px-5 py-4">
            <div>
              <DialogP.Title className="text-[1.05rem] font-semibold">{title}</DialogP.Title>
              {description && <DialogP.Description className="mt-1 text-sm text-muted">{description}</DialogP.Description>}
            </div>
            <DialogP.Close asChild>
              <button className="rounded-md p-1 text-muted hover:bg-white/5 hover:text-fg" aria-label="Close">
                <X className="size-4" />
              </button>
            </DialogP.Close>
          </div>
          <div className="max-h-[60vh] overflow-y-auto px-5 py-4">{children}</div>
          {footer && <div className="flex justify-end gap-2 border-t border-line bg-bg-2/50 px-5 py-3">{footer}</div>}
        </DialogP.Content>
      </DialogP.Portal>
    </DialogP.Root>
  );
}

export function Confirm({
  open, onOpenChange, title, message, confirmLabel = "Confirm", danger, onConfirm,
}: {
  open: boolean; onOpenChange: (v: boolean) => void; title: string; message: ReactNode; confirmLabel?: string;
  danger?: boolean; onConfirm: () => void;
}) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      width={440}
      footer={
        <>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button
            variant={danger ? "danger" : "primary"}
            onClick={() => {
              onConfirm();
              onOpenChange(false);
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="text-sm text-fg-2">{message}</div>
    </Dialog>
  );
}

export function Progress({ value, tone = "accent", className, indeterminate }: {
  value: number; tone?: "accent" | "ok" | "err" | "mon"; className?: string; indeterminate?: boolean;
}) {
  const c = { accent: "bg-accent", ok: "bg-ok", err: "bg-err", mon: "bg-mon" }[tone];
  return (
    <div className={cn("h-1.5 w-full overflow-hidden rounded-full bg-line", className)}>
      <div
        className={cn("h-full rounded-full transition-[width] duration-300", c, indeterminate && "stripes animate-pulse-soft")}
        style={{ width: `${Math.max(2, Math.min(100, value * 100))}%` }}
      />
    </div>
  );
}

export function Meter({ value, vertical, className }: { value: number; vertical?: boolean; className?: string }) {
  const l = meterLevel(value);
  return (
    <div className={cn("relative overflow-hidden rounded-sm bg-[#0a0b0d]", vertical ? "w-2" : "h-2", className)}>
      <div
        className={cn("absolute", vertical ? "meter-gradient-v bottom-0 left-0 right-0" : "meter-gradient left-0 top-0 bottom-0")}
        style={vertical ? { height: "100%", clipPath: `inset(${(1 - l) * 100}% 0 0 0)` } : { width: "100%", clipPath: `inset(0 ${(1 - l) * 100}% 0 0)` }}
      />
    </div>
  );
}

export function Card({ children, className, title, actions, subtitle }: {
  children: ReactNode; className?: string; title?: ReactNode; actions?: ReactNode; subtitle?: ReactNode;
}) {
  return (
    <section className={cn("card", className)}>
      {(title || actions) && (
        <header className="flex items-center justify-between gap-3 px-4 pt-3.5 pb-1">
          <div className="min-w-0">
            {title && <h3 className="truncate text-[0.95rem] font-semibold">{title}</h3>}
            {subtitle && <p className="mt-0.5 text-xs text-muted">{subtitle}</p>}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
        </header>
      )}
      <div className="p-4 pt-3">{children}</div>
    </section>
  );
}

export function Empty({ icon, title, children, action }: { icon?: ReactNode; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center px-6 py-16 text-center">
      {icon && <div className="mb-4 grid size-14 place-items-center rounded-2xl border border-line bg-panel text-muted">{icon}</div>}
      <h3 className="text-base font-semibold">{title}</h3>
      {children && <p className="mt-1.5 max-w-md text-sm text-muted">{children}</p>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}

export function Field({ label, hint, children, className }: { label: ReactNode; hint?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <label className={cn("block", className)}>
      <span className="label mb-1.5 block">{label}</span>
      {children}
      {hint && <span className="mt-1 block text-xs text-muted">{hint}</span>}
    </label>
  );
}

export function Row({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 py-2.5">
      <div className="min-w-0">
        <div className="text-[0.9rem] text-fg">{label}</div>
        {hint && <div className="mt-0.5 text-xs text-muted">{hint}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

export function PageHeader({ title, subtitle, actions, children }: { title: ReactNode; subtitle?: ReactNode; actions?: ReactNode; children?: ReactNode }) {
  return (
    <div className="drag sticky top-0 z-10 border-b border-line/70 bg-bg/85 px-8 pt-7 pb-4 backdrop-blur-md">
      <div className="flex items-end justify-between gap-4">
        <div className="no-drag min-w-0">
          <h1 className="truncate text-[1.6rem] font-semibold tracking-tight">{title}</h1>
          {subtitle && <p className="mt-0.5 text-sm text-muted">{subtitle}</p>}
        </div>
        {actions && <div className="no-drag flex shrink-0 items-center gap-2">{actions}</div>}
      </div>
      {children && <div className="no-drag mt-4">{children}</div>}
    </div>
  );
}

export function StatusDot({ tone }: { tone: "ok" | "warn" | "err" | "idle" | "live" }) {
  const c = { ok: "bg-ok", warn: "bg-warn", err: "bg-err", idle: "bg-faint", live: "bg-live animate-pulse-soft" }[tone];
  return <span className={cn("inline-block size-2 shrink-0 rounded-full", c)} />;
}
