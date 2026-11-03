import { useEffect, useRef, type ReactNode } from 'react'

interface ModalProps {
  labelledBy: string
  onClose: () => void
  children: ReactNode
  role?: 'dialog' | 'alertdialog'
  className?: string
}

const FOCUSABLE_SELECTOR = 'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])'

export function Modal({ labelledBy, onClose, children, role = 'dialog', className = '' }: ModalProps) {
  const panelRef = useRef<HTMLElement>(null)
  const onCloseRef = useRef(onClose)

  useEffect(() => { onCloseRef.current = onClose }, [onClose])

  useEffect(() => {
    const panel = panelRef.current
    if (!panel) return

    const autofocus = panel.querySelector<HTMLElement>('[autofocus]')
    if (autofocus) autofocus.focus()
    else panel.querySelector<HTMLElement>(FOCUSABLE_SELECTOR)?.focus()

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault()
        onCloseRef.current()
        return
      }
      if (event.key !== 'Tab' || !panel) return

      const focusable = Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
      if (focusable.length === 0) {
        event.preventDefault()
        panel.focus()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }

    panel.addEventListener('keydown', handleKeyDown)
    return () => panel.removeEventListener('keydown', handleKeyDown)
  }, [])

  return (
    <div className="modal-backdrop">
      <section ref={panelRef} tabIndex={-1} role={role} aria-modal="true" aria-labelledby={labelledBy} className={`modal-panel ${className}`}>
        {children}
      </section>
    </div>
  )
}
