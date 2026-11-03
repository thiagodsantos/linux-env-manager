import type { ButtonHTMLAttributes, ReactNode } from 'react'

export type ButtonVariant = 'primary' | 'secondary' | 'danger' | 'quiet'

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
  icon?: ReactNode
}

const variants: Record<ButtonVariant, string> = {
  primary: 'bg-ink text-surface hover:bg-ink/90 shadow-sm',
  secondary: 'border border-line bg-surface text-ink hover:bg-canvas/70',
  danger: 'bg-danger text-surface hover:bg-ink shadow-sm',
  quiet: 'text-muted hover:bg-canvas/80 hover:text-ink',
}

export function Button({ variant = 'secondary', icon, className = '', children, ...props }: ButtonProps) {
  return (
    <button
      className={`inline-flex min-h-10 items-center justify-center gap-2 rounded-xl px-4 py-2 text-sm font-semibold transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:cursor-not-allowed disabled:opacity-50 ${variants[variant]} ${className}`}
      {...props}
    >
      {icon}
      {children}
    </button>
  )
}
