import { useState } from 'react'
import { Eye, EyeOff, Pencil, Trash2 } from 'lucide-react'
import { Button } from '@/components/atoms/Button'
import type { Variable } from '@/types'

interface VariableRowProps {
  variable: Variable
  profileName: string
  onEdit: (variable: Variable) => void
  onDelete: (variable: Variable) => void
}

export function VariableRow({ variable, profileName, onEdit, onDelete }: VariableRowProps) {
  const [revealed, setRevealed] = useState(false)

  return (
    <article className="group flex flex-col gap-3 border-b border-line/70 px-5 py-4 last:border-b-0 sm:flex-row sm:items-center sm:justify-between sm:px-6">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <code className="truncate font-mono text-sm font-bold text-ink">{variable.key}</code>
          <span className="rounded-md bg-surface px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wide text-muted">{profileName === 'General' ? 'shared' : variable.inherited ? 'inherited' : 'profile'}</span>
        </div>
        <p className="mt-1.5 max-w-2xl truncate font-mono text-sm text-muted" aria-label={`${variable.key} value`}>
          {revealed ? variable.value : '••••••••••••'}
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-1 self-end sm:self-auto">
        <Button
          type="button"
          variant="quiet"
          className="variable-action"
          aria-label={`${revealed ? 'Hide' : 'Show'} ${variable.key} value`}
          onClick={() => setRevealed((current) => !current)}
        >
          {revealed ? <EyeOff aria-hidden="true" size={17} /> : <Eye aria-hidden="true" size={17} />}
        </Button>
        <Button type="button" variant="quiet" className="variable-action" aria-label={`Edit ${variable.key}`} onClick={() => onEdit(variable)}>
          <Pencil aria-hidden="true" size={16} />
        </Button>
        <Button type="button" variant="quiet" className="variable-action variable-action-danger" aria-label={`Delete ${variable.key}`} title={variable.inherited ? 'Inherited values cannot be deleted from this profile' : undefined} disabled={variable.inherited} onClick={() => onDelete(variable)}>
          <Trash2 aria-hidden="true" size={17} />
        </Button>
      </div>
    </article>
  )
}
