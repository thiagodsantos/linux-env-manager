import { useCallback, useEffect, useMemo, useState, type FormEvent } from 'react'
import { ArrowDownToLine, ArrowUpFromLine, Check, CircleAlert, FileText, Plus, Shield, Sparkles, Terminal, Trash2, X } from 'lucide-react'
import { shellIntegrationApi, variableApi } from '@/api'
import { Button } from '@/components/atoms/Button'
import { Modal } from '@/components/atoms/Modal'
import { VariableCollection } from '@/components/organisms/VariableCollection'
import { WorkspaceLayout } from '@/components/templates/WorkspaceLayout'
import type { ImportPreview, ImportResult, ShellIntegrationStatus, Variable } from '@/types'

const KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/
const SHELL_INTEGRATIONS: { shell: ShellIntegrationStatus['shell']; label: string; startupFile: ShellIntegrationStatus['startupFile'] }[] = [
  { shell: 'bash', label: 'Bash', startupFile: '~/.bashrc' },
  { shell: 'zsh', label: 'Zsh', startupFile: '~/.zshrc' },
]

function messageFromError(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

function VariableEditor({
  variable,
  variables,
  onClose,
  onSaved,
}: {
  variable: Variable | null
  variables: Variable[]
  onClose: () => void
  onSaved: () => Promise<void>
}) {
  const [key, setKey] = useState(variable?.key ?? '')
  const [value, setValue] = useState(variable?.value ?? '')
  const [keyError, setKeyError] = useState('')
  const [formError, setFormError] = useState('')
  const [saving, setSaving] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setKeyError('')
    setFormError('')
    if (!KEY_PATTERN.test(key)) {
      setKeyError('Use a letter or underscore first, then only letters, numbers, or underscores.')
      return
    }
    if (variables.some((item) => item.key === key && item.key !== variable?.key)) {
      setKeyError('A variable with this key already exists. Edit it instead.')
      return
    }
    setSaving(true)
    try {
      await variableApi.save({ key, value, originalKey: variable?.key ?? null })
      onClose()
      await onSaved()
    } catch (error) {
      setFormError(messageFromError(error, 'Unable to save this variable.'))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Modal labelledBy="editor-title" onClose={onClose}>
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-xs font-bold uppercase tracking-[0.16em] text-red">{variable ? 'Make a change' : 'Add to your workspace'}</p>
            <h2 id="editor-title" className="mt-1 text-xl font-bold tracking-tight">{variable ? 'Edit variable' : 'New variable'}</h2>
          </div>
          <Button type="button" variant="quiet" className="size-9 min-h-9 px-0" aria-label="Close editor" onClick={onClose}><X size={18} /></Button>
        </div>
        <form className="mt-6 space-y-4" onSubmit={(event) => void submit(event)}>
          <div>
            <label className="field-label" htmlFor="variable-key">Key</label>
            <input autoFocus id="variable-key" className="field-input font-mono" value={key} onChange={(event) => setKey(event.target.value)} aria-invalid={Boolean(keyError)} aria-describedby={keyError ? 'key-error' : 'key-hint'} />
            {keyError ? <p id="key-error" className="field-error" role="alert">{keyError}</p> : <p id="key-hint" className="field-hint">Letters, numbers, and underscores; cannot start with a number.</p>}
          </div>
          <div>
            <label className="field-label" htmlFor="variable-value">Value</label>
            <textarea id="variable-value" rows={3} className="field-input resize-y font-mono" value={value} onChange={(event) => setValue(event.target.value)} />
            <p className="field-hint">Stored privately in this app's user configuration.</p>
          </div>
          {formError && <p role="alert" className="rounded-xl bg-red/10 px-3 py-2.5 text-sm font-medium text-red">{formError}</p>}
          <div className="flex justify-end gap-2 border-t border-line pt-4">
            <Button type="button" onClick={onClose}>Cancel</Button>
            <Button type="submit" variant="primary" disabled={saving} icon={<Check size={16} />}>{saving ? 'Saving…' : 'Save variable'}</Button>
          </div>
        </form>
    </Modal>
  )
}

function DeleteConfirmation({
  variable,
  onCancel,
  onConfirm,
}: {
  variable: Variable
  onCancel: () => void
  onConfirm: () => Promise<void>
}) {
  const [error, setError] = useState('')
  const [deleting, setDeleting] = useState(false)

  async function confirm() {
    setDeleting(true)
    setError('')
    try {
      await onConfirm()
    } catch (reason) {
      setError(messageFromError(reason, 'Unable to delete this variable.'))
      setDeleting(false)
    }
  }

  return (
    <Modal role="alertdialog" labelledBy="delete-title" onClose={onCancel} className="max-w-md">
        <div className="grid size-11 place-items-center rounded-2xl bg-red/10 text-red"><CircleAlert aria-hidden="true" size={22} /></div>
        <h2 id="delete-title" className="mt-4 text-xl font-bold">Delete this variable?</h2>
        <p className="mt-2 text-sm leading-6 text-ink-muted">This permanently removes <code className="rounded bg-blue-soft px-1.5 py-0.5 font-mono font-bold text-ink">{variable.key}</code> from this app's saved variables.</p>
        {error && <p role="alert" className="mt-4 rounded-xl bg-red/10 px-3 py-2.5 text-sm font-medium text-red">{error}</p>}
        <div className="mt-6 flex justify-end gap-2">
          <Button type="button" onClick={onCancel} disabled={deleting}>Keep variable</Button>
          <Button type="button" variant="danger" disabled={deleting} icon={<Trash2 aria-hidden="true" size={15} />} onClick={() => void confirm()}>{deleting ? 'Deleting…' : 'Delete variable'}</Button>
        </div>
    </Modal>
  )
}


function ImportDialog({
  preview,
  onClose,
  onApply,
}: {
  preview: ImportPreview
  onClose: () => void
  onApply: (result: ImportResult) => void
}) {
  const [error, setError] = useState('')
  const [applying, setApplying] = useState(false)
  const canApply = preview.invalidLines.length === 0 && preview.added.length + preview.updated.length > 0

  function close() {
    void variableApi.discardImport(preview.token).catch(() => undefined)
    onClose()
  }

  async function apply() {
    setApplying(true)
    setError('')
    try {
      onApply(await variableApi.applyImport(preview.token))
    } catch (reason) {
      setError(messageFromError(reason, 'Unable to apply this import.'))
      setApplying(false)
    }
  }

  return (
    <Modal labelledBy="import-title" onClose={close} className="max-w-2xl">
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-start gap-3">
            <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-blue-soft text-ink"><FileText size={19} /></div>
            <div><p className="text-xs font-bold uppercase tracking-[0.16em] text-red">Review before saving</p><h2 id="import-title" className="mt-1 text-xl font-bold">Import preview</h2></div>
          </div>
          <Button type="button" variant="quiet" className="size-9 min-h-9 px-0" aria-label="Close import preview" onClick={close}><X size={18} /></Button>
        </div>
        <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3">
          <div className="count-card"><span className="count-number">{preview.added.length}</span><span className="count-label">New keys</span></div>
          <div className="count-card"><span className="count-number">{preview.updated.length}</span><span className="count-label">Will update</span></div>
          <div className="count-card col-span-2 sm:col-span-1"><span className="count-number">{preview.invalidLines.length}</span><span className="count-label">Invalid lines</span></div>
        </div>
        {preview.added.length + preview.updated.length > 0 && (
          <div className="mt-5 max-h-40 overflow-auto rounded-xl border border-line bg-white px-4 py-3 text-sm">
            {preview.added.map((key) => <p key={`new-${key}`} className="flex items-center gap-2 py-1"><span className="size-2 rounded-full bg-green-dot" /> <code className="font-mono">{key}</code><span className="ml-auto text-xs text-ink-muted">new</span></p>)}
            {preview.updated.map((key) => <p key={`updated-${key}`} className="flex items-center gap-2 py-1"><span className="size-2 rounded-full bg-blue" /> <code className="font-mono">{key}</code><span className="ml-auto text-xs text-ink-muted">update</span></p>)}
          </div>
        )}
        {preview.invalidLines.length > 0 && (
          <div className="mt-4 rounded-xl border border-red/20 bg-red/5 p-4">
            <p className="text-sm font-bold text-red">Fix these lines before importing</p>
            <ul className="mt-2 space-y-1.5 text-sm text-ink-muted">{preview.invalidLines.map((line) => <li key={line.line}>Line {line.line}: {line.message}</li>)}</ul>
            <p className="mt-3 text-xs leading-5 text-ink-muted">No changes will be applied while any invalid line remains.</p>
          </div>
        )}
        {preview.invalidLines.length === 0 && preview.added.length + preview.updated.length === 0 && <p className="mt-5 rounded-xl bg-blue-soft/60 p-4 text-sm text-ink-muted">This file contains no variable changes to apply.</p>}
        {error && <p role="alert" className="mt-4 rounded-xl bg-red/10 px-3 py-2.5 text-sm font-medium text-red">{error}</p>}
        <div className="mt-6 flex justify-end gap-2 border-t border-line pt-4">
          <Button type="button" onClick={close}>Cancel</Button>
          <Button type="button" variant="primary" disabled={!canApply || applying} icon={<ArrowUpFromLine size={16} />} onClick={() => void apply()}>{applying ? 'Applying…' : 'Apply import'}</Button>
        </div>
    </Modal>
  )
}

export default function VariablesPage() {
  const [variables, setVariables] = useState<Variable[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState('')
  const [search, setSearch] = useState('')
  const [editor, setEditor] = useState<Variable | null | undefined>(undefined)
  const [deleting, setDeleting] = useState<Variable | null>(null)
  const [preview, setPreview] = useState<ImportPreview | null>(null)
  const [importing, setImporting] = useState(false)
  const [actionError, setActionError] = useState('')
  const [notice, setNotice] = useState('')
  const [shellIntegrationStatuses, setShellIntegrationStatuses] = useState<ShellIntegrationStatus[]>([])
  const [shellIntegrationLoading, setShellIntegrationLoading] = useState(true)
  const [shellIntegrationLoadError, setShellIntegrationLoadError] = useState('')
  const [shellIntegrationActionError, setShellIntegrationActionError] = useState('')
  const [shellIntegrationNotice, setShellIntegrationNotice] = useState('')
  const [shellIntegrationActionShell, setShellIntegrationActionShell] = useState<ShellIntegrationStatus['shell'] | null>(null)

  const refreshShellIntegrations = useCallback(async () => {
    setShellIntegrationLoading(true)
    setShellIntegrationLoadError('')
    try {
      setShellIntegrationStatuses(await shellIntegrationApi.getStatus())
    } catch (error) {
      setShellIntegrationLoadError(messageFromError(error, 'Unable to load shell integration status.'))
    } finally {
      setShellIntegrationLoading(false)
    }
  }, [])

  const refresh = useCallback(async () => {
    setLoading(true)
    setLoadError('')
    try {
      setVariables(await variableApi.list())
    } catch (error) {
      setLoadError(messageFromError(error, 'Unable to load saved variables.'))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    void shellIntegrationApi.getStatus()
      .then((statuses) => {
        if (cancelled) return
        setShellIntegrationStatuses(statuses)
        setShellIntegrationLoadError('')
      })
      .catch((error: unknown) => {
        if (!cancelled) setShellIntegrationLoadError(messageFromError(error, 'Unable to load shell integration status.'))
      })
      .finally(() => {
        if (!cancelled) setShellIntegrationLoading(false)
      })
    return () => { cancelled = true }
  }, [])

  useEffect(() => {
    let cancelled = false
    void variableApi.list()
      .then((loadedVariables) => {
        if (cancelled) return
        setVariables(loadedVariables)
        setLoadError('')
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoadError(messageFromError(error, 'Unable to load saved variables.'))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => { cancelled = true }
  }, [])

  async function updateShellIntegration(shell: ShellIntegrationStatus['shell'], enabled: boolean) {
    const label = shell === 'bash' ? 'Bash' : 'Zsh'
    setShellIntegrationActionShell(shell)
    setShellIntegrationActionError('')
    setShellIntegrationNotice('')
    try {
      await shellIntegrationApi.set({ shell, enabled })
      setShellIntegrationNotice(`${label} shell integration ${enabled ? 'enabled' : 'disabled'}.`)
    } catch (error) {
      setShellIntegrationActionError(messageFromError(error, `Unable to ${enabled ? 'enable' : 'disable'} ${label} integration.`))
    }
    await refreshShellIntegrations()
    setShellIntegrationActionShell(null)
  }

  const filteredVariables = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return needle ? variables.filter((variable) => variable.key.toLowerCase().includes(needle)) : variables
  }, [search, variables])

  async function startImport() {
    setImporting(true)
    setActionError('')
    setNotice('')
    try {
      const result = await variableApi.previewImport()
      if (result) setPreview(result)
    } catch (error) {
      setActionError(messageFromError(error, 'Unable to open an environment file.'))
    } finally {
      setImporting(false)
    }
  }

  async function removeVariable() {
    if (!deleting) return
    await variableApi.remove(deleting.key)
    setDeleting(null)
    await refresh()
  }

  function imported(result: ImportResult) {
    setPreview(null)
    setNotice(`Import complete: ${result.added.length} added, ${result.updated.length} updated.`)
    void refresh()
  }

  return (
    <WorkspaceLayout>
      <section className="mb-9 grid gap-8 lg:grid-cols-[1fr_260px] lg:items-end">
        <div>
          <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-line bg-white/75 px-3 py-1.5 text-xs font-semibold text-ink-muted"><Sparkles aria-hidden="true" size={14} className="text-red" /> Your environment, organized</div>
          <h1 className="max-w-2xl text-3xl font-bold leading-tight tracking-[-0.04em] sm:text-4xl">A calmer place for your <span className="text-red">environment variables.</span></h1>
          <p className="mt-3 max-w-2xl text-sm leading-6 text-ink-muted sm:text-base">Keep the values you use across projects in one private, user-scoped workspace.</p>
        </div>
        <div className="rounded-2xl border border-line bg-white/80 p-4 shadow-card">
          <div className="flex items-start gap-3"><div className="mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg bg-blue-soft text-ink"><Shield size={16} /></div><div><p className="text-sm font-bold">Saved here, not everywhere</p><p className="mt-1 text-xs leading-5 text-ink-muted">Changes are stored by this app. They do not alter running shells or processes.</p></div></div>
        </div>
      </section>

      <section aria-labelledby="variables-heading" className="overflow-hidden rounded-3xl border border-line bg-white shadow-card">
        <div className="flex flex-col gap-4 border-b border-line px-5 py-5 sm:flex-row sm:items-center sm:justify-between sm:px-6">
          <div><div className="flex items-center gap-2"><h2 id="variables-heading" className="text-lg font-bold tracking-tight">Your variables</h2><span className="rounded-full bg-cream px-2.5 py-1 text-xs font-bold text-ink-muted">{variables.length}</span></div><p className="mt-1 text-sm text-ink-muted">Only visible to your user account</p></div>
          <div className="flex flex-wrap gap-2">
            <Button type="button" onClick={() => void startImport()} disabled={importing} icon={<ArrowDownToLine size={16} />}>{importing ? 'Choosing file…' : 'Import .env'}</Button>
            <Button type="button" variant="primary" onClick={() => { setActionError(''); setEditor(null) }} icon={<Plus size={17} />}>Add variable</Button>
          </div>
        </div>
        {notice && <div role="status" className="flex items-center gap-2 border-b border-line bg-green-soft px-6 py-3 text-sm font-semibold text-green-text"><Check size={16} />{notice}<button type="button" aria-label="Dismiss message" className="ml-auto rounded p-1 hover:bg-white/70" onClick={() => setNotice('')}><X size={15} /></button></div>}
        {actionError && <div role="alert" className="flex items-start gap-2 border-b border-line bg-red/5 px-6 py-3 text-sm text-red"><CircleAlert size={16} className="mt-0.5 shrink-0" />{actionError}<button type="button" aria-label="Dismiss error" className="ml-auto rounded p-1 hover:bg-white/70" onClick={() => setActionError('')}><X size={15} /></button></div>}
        <VariableCollection
          variables={variables}
          filteredVariables={filteredVariables}
          loading={loading}
          loadError={loadError}
          search={search}
          onSearch={setSearch}
          onRetry={() => void refresh()}
          onAdd={() => setEditor(null)}
          onEdit={setEditor}
          onDelete={setDeleting}
        />
      </section>

      <section aria-labelledby="shell-integration-heading" className="mt-6 overflow-hidden rounded-3xl border border-line bg-white shadow-card">
        <div className="border-b border-line px-5 py-5 sm:px-6">
          <div className="flex items-start gap-3">
            <div className="mt-0.5 grid size-9 shrink-0 place-items-center rounded-xl bg-blue-soft text-ink"><Terminal aria-hidden="true" size={18} /></div>
            <div>
              <h2 id="shell-integration-heading" className="text-lg font-bold tracking-tight">Shell integration</h2>
              <p className="mt-1 max-w-3xl text-sm leading-6 text-ink-muted">Opt in separately for each shell to make your managed variables available in new terminal sessions, for example with <code className="rounded bg-cream px-1 font-mono text-xs">echo $VAR_NAME</code>.</p>
            </div>
          </div>
          <p className="mt-4 rounded-xl bg-cream/70 px-4 py-3 text-sm leading-6 text-ink-muted">Enabling or disabling integration edits only this app's managed block in your user startup file—not other settings. It affects new shells only. To load the change in an existing shell, source its file (for example, <code className="font-mono text-xs">source ~/.bashrc</code> or <code className="font-mono text-xs">source ~/.zshrc</code>).</p>
        </div>
        {shellIntegrationLoadError && <div role="alert" className="flex flex-wrap items-center gap-3 border-b border-line bg-red/5 px-5 py-3 text-sm text-red sm:px-6"><CircleAlert aria-hidden="true" size={16} />{shellIntegrationLoadError}<Button type="button" className="ml-auto" onClick={() => void refreshShellIntegrations()}>Retry shell status</Button></div>}
        {shellIntegrationActionError && <p role="alert" className="border-b border-line bg-red/5 px-5 py-3 text-sm text-red sm:px-6">{shellIntegrationActionError}</p>}
        {shellIntegrationNotice && <p role="status" className="border-b border-line bg-green-soft px-5 py-3 text-sm font-semibold text-green-text sm:px-6">{shellIntegrationNotice}</p>}
        <div className="grid gap-3 p-4 sm:grid-cols-2 sm:p-5">
          {SHELL_INTEGRATIONS.map(({ shell, label, startupFile }) => {
            const status = shellIntegrationStatuses.find((integration) => integration.shell === shell)
            const busy = shellIntegrationLoading || shellIntegrationActionShell !== null
            return (
              <article key={shell} className="rounded-2xl border border-line bg-white p-4">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h3 className="font-bold">{label}</h3>
                    <p className="mt-1 text-xs text-ink-muted">Managed file: <code className="font-mono">{status?.startupFile ?? startupFile}</code></p>
                  </div>
                  <p className={`rounded-full px-2.5 py-1 text-xs font-bold ${status?.enabled ? 'bg-green-soft text-green-text' : 'bg-cream text-ink-muted'}`}>
                    {shellIntegrationLoading ? 'Checking…' : status ? (status.enabled ? 'Enabled' : 'Disabled') : 'Unavailable'}
                  </p>
                </div>
                <Button
                  type="button"
                  className="mt-4 w-full"
                  variant={status?.enabled ? 'danger' : 'primary'}
                  disabled={busy || !status}
                  onClick={() => status && void updateShellIntegration(shell, !status.enabled)}
                >
                  {shellIntegrationActionShell === shell ? (status?.enabled ? 'Disabling…' : 'Enabling…') : `${status?.enabled ? 'Disable' : 'Enable'} ${label} integration`}
                </Button>
              </article>
            )
          })}
        </div>
      </section>
      <p className="mt-4 flex items-start gap-2 px-1 text-xs leading-5 text-ink-muted"><Shield aria-hidden="true" size={14} className="mt-0.5 shrink-0" />Values are masked until you choose to reveal them. The app never runs the contents of imported files.</p>

      {editor !== undefined && <VariableEditor variable={editor} variables={variables} onClose={() => setEditor(undefined)} onSaved={refresh} />}
      {deleting && <DeleteConfirmation variable={deleting} onCancel={() => setDeleting(null)} onConfirm={removeVariable} />}
      {preview && <ImportDialog preview={preview} onClose={() => setPreview(null)} onApply={imported} />}
    </WorkspaceLayout>
  )
}
