import type { ReactNode } from 'react'
import { KeyRound, ShieldCheck } from 'lucide-react'

interface WorkspaceLayoutProps {
  children: ReactNode
}

export function WorkspaceLayout({ children }: WorkspaceLayoutProps) {
  return (
    <div className="min-h-screen bg-cream text-ink">
      <header className="border-b border-line/70 bg-white/70">
        <div className="mx-auto flex max-w-6xl items-center justify-between px-5 py-5 sm:px-8">
          <div className="flex items-center gap-3">
            <div className="grid size-11 place-items-center rounded-2xl bg-ink text-white shadow-sm">
              <KeyRound aria-hidden="true" size={21} strokeWidth={2.2} />
            </div>
            <div>
              <p className="text-[11px] font-bold uppercase tracking-[0.19em] text-red">LINUX UTILITIES</p>
              <p className="mt-0.5 text-base font-bold tracking-tight">Environment manager</p>
            </div>
          </div>
          <span className="hidden items-center gap-2 rounded-full border border-line bg-white px-3 py-2 text-xs font-semibold text-ink-muted sm:inline-flex">
            <ShieldCheck aria-hidden="true" size={15} className="text-ink" />
            Private to this user
          </span>
        </div>
      </header>
      <main className="mx-auto max-w-6xl px-5 pb-12 pt-9 sm:px-8 sm:pt-12">
        {children}
        <footer className="mt-12 border-t border-line/70 pt-5 text-center text-xs leading-5 text-ink-muted">
          Saved values are user-scoped. Existing shells and processes are unchanged; opt in to Bash/Zsh integration below for new terminal sessions.
        </footer>
      </main>
    </div>
  )
}
