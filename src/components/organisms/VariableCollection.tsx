import { CircleAlert, FileText, Plus, Search } from 'lucide-react'
import { Button } from '@/components/atoms/Button'
import { VariableRow } from '@/components/molecules/VariableRow'
import type { Variable } from '@/types'

interface VariableCollectionProps {
  variables: Variable[]
  filteredVariables: Variable[]
  loading: boolean
  loadError: string
  search: string
  onSearch: (value: string) => void
  onRetry: () => void
  onAdd: () => void
  onEdit: (variable: Variable) => void
  onDelete: (variable: Variable) => void
}

export function VariableCollection({
  variables,
  filteredVariables,
  loading,
  loadError,
  search,
  onSearch,
  onRetry,
  onAdd,
  onEdit,
  onDelete,
}: VariableCollectionProps) {
  return (
    <>
      {!loading && !loadError && variables.length > 0 && (
        <div className="border-b border-line bg-canvas/35 px-5 py-3 sm:px-6">
          <label className="relative block max-w-sm">
            <span className="sr-only">Search variable keys</span>
            <Search size={16} aria-hidden="true" className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
            <input className="field-input search-input h-10 py-2" placeholder="Search keys…" value={search} onChange={(event) => onSearch(event.target.value)} />
          </label>
        </div>
      )}
      {loading ? (
        <div role="status" className="px-6 py-16 text-center text-sm font-medium text-muted">Loading your saved variables…</div>
      ) : loadError ? (
        <div className="px-6 py-14 text-center">
          <CircleAlert className="mx-auto text-danger" size={26} />
          <p className="mt-3 font-semibold">Couldn't load your variables</p>
          <p className="mt-1 text-sm text-muted">{loadError}</p>
          <Button className="mt-4" onClick={onRetry}>Try again</Button>
        </div>
      ) : variables.length === 0 ? (
        <div className="px-6 py-14 text-center">
          <div className="mx-auto grid size-12 place-items-center rounded-2xl bg-surface text-ink"><FileText size={22} /></div>
          <h3 className="mt-4 font-bold">A fresh start</h3>
          <p className="mx-auto mt-1 max-w-sm text-sm leading-6 text-muted">Add your first variable or import an existing .env file. Your saved values stay on this device.</p>
          <Button className="mt-5" variant="primary" onClick={onAdd} icon={<Plus size={16} />}>Add your first variable</Button>
        </div>
      ) : filteredVariables.length === 0 ? (
        <p className="px-6 py-12 text-center text-sm text-muted">No keys match “{search}”.</p>
      ) : (
        <div className="max-h-96 overflow-y-auto overscroll-contain">{filteredVariables.map((variable) => <VariableRow key={variable.key} variable={variable} onEdit={onEdit} onDelete={onDelete} />)}</div>
      )}
    </>
  )
}
