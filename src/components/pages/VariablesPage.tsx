import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent, type KeyboardEvent } from 'react'
import { ArrowDownToLine, ArrowUpFromLine, Check, ChevronDown, CircleAlert, FileText, Plus, Shield, Sparkles, Terminal, Trash2, X } from 'lucide-react'
import { profileApi, shellIntegrationApi, variableApi } from '@/api'
import { Button } from '@/components/atoms/Button'
import { Modal } from '@/components/atoms/Modal'
import { VariableCollection } from '@/components/organisms/VariableCollection'
import { WorkspaceLayout } from '@/components/templates/WorkspaceLayout'
import type { CreateProfileInput, ImportPreview, ImportResult, Profile, ShellIntegrationStatus, Variable } from '@/types'

const GENERAL_PROFILE = 'General'
const KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/
const SHELL_INTEGRATIONS: { shell: ShellIntegrationStatus['shell']; label: string; startupFile: ShellIntegrationStatus['startupFile'] }[] = [
  { shell: 'bash', label: 'Bash', startupFile: '~/.bashrc' },
  { shell: 'zsh', label: 'Zsh', startupFile: '~/.zshrc' },
]

function messageFromError(error: unknown, fallback: string): string {
  if (typeof error === 'string' && error.trim()) return error
  if (error instanceof Error && error.message) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = error.message
    if (typeof message === 'string' && message.trim()) return message
  }
  return fallback
}

function ProfileEditor({ profiles, onClose, onCreated }: { profiles: Profile[]; onClose: () => void; onCreated: (profile: Profile) => Promise<void> }) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [color, setColor] = useState('#05668d')
  const [importFrom, setImportFrom] = useState('')
  const [error, setError] = useState('')
  const [saving, setSaving] = useState(false)

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const normalizedName = name.trim()
    setError('')
    if (!normalizedName) { setError('Enter a profile name.'); return }
    if (normalizedName.toLowerCase() === GENERAL_PROFILE.toLowerCase()) { setError('General is reserved for the shared profile.'); return }
    if (profiles.some((profile) => profile.name.toLowerCase() === normalizedName.toLowerCase())) { setError('A profile with this name already exists.'); return }
    setSaving(true)
    try {
      const input: CreateProfileInput = { name: normalizedName, description: description.trim(), color, importFrom: importFrom || null }
      await profileApi.create(input)
      await onCreated({ name: input.name, description: input.description, color: input.color })
    } catch (reason) {
      setError(messageFromError(reason, 'Unable to create this profile.'))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Modal labelledBy="profile-editor-title" onClose={onClose}>
      <div className="flex items-start justify-between gap-4">
        <div><p className="text-xs font-bold uppercase tracking-[0.16em] text-danger">A focused workspace</p><h2 id="profile-editor-title" className="mt-1 text-xl font-bold">Create profile</h2></div>
        <Button type="button" variant="quiet" className="size-9 min-h-9 px-0" aria-label="Close profile form" onClick={onClose}><X size={18} /></Button>
      </div>
      <form className="mt-6 space-y-4" onSubmit={(event) => void submit(event)}>
        <div><label className="field-label" htmlFor="profile-name">Profile name</label><input autoFocus id="profile-name" className="field-input" value={name} onChange={(event) => setName(event.target.value)} aria-describedby={error ? 'profile-error' : undefined} /></div>
        <div><label className="field-label" htmlFor="profile-description">Description</label><textarea id="profile-description" rows={2} className="field-input resize-y" value={description} onChange={(event) => setDescription(event.target.value)} /></div>
        <div><label className="field-label" htmlFor="profile-color">Profile color</label><input id="profile-color" type="color" className="field-input h-12 cursor-pointer p-1" value={color} onChange={(event) => setColor(event.target.value)} /></div>
        <div><label className="field-label" htmlFor="profile-import-from">Start with variables from</label><select id="profile-import-from" className="field-input" value={importFrom} onChange={(event) => setImportFrom(event.target.value)}><option value="">No profile (empty)</option>{[ { name: GENERAL_PROFILE }, ...profiles.filter((profile) => profile.name !== GENERAL_PROFILE) ].map((profile) => <option key={profile.name} value={profile.name}>{profile.name}</option>)}</select><p className="field-hint">Imported values become the new profile's starting variables.</p></div>
        {error && <p id="profile-error" role="alert" className="rounded-xl bg-danger/10 px-3 py-2.5 text-sm font-medium text-danger">{error}</p>}
        <div className="flex justify-end gap-2 border-t border-line pt-4"><Button type="button" onClick={onClose}>Cancel</Button><Button type="submit" variant="primary" disabled={saving}>{saving ? 'Creating…' : 'Create profile'}</Button></div>
      </form>
    </Modal>
  )
}

function VariableEditor({
  profileName,
  variable,
  variables,
  onClose,
  onSaved,
}: {
  profileName: string
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
      await variableApi.save({ profileName, key, value, originalKey: variable?.key ?? null })
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
            <p className="text-xs font-bold uppercase tracking-[0.16em] text-danger">{variable ? 'Make a change' : 'Add to your workspace'}</p>
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
            <p className="field-hint">{variable?.inherited ? 'Saving creates a profile-local override; the shared General value remains unchanged.' : 'Stored privately in this app\'s user configuration.'}</p>
          </div>
          {formError && <p role="alert" className="rounded-xl bg-danger/10 px-3 py-2.5 text-sm font-medium text-danger">{formError}</p>}
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
        <div className="grid size-11 place-items-center rounded-2xl bg-danger/10 text-danger"><CircleAlert aria-hidden="true" size={22} /></div>
        <h2 id="delete-title" className="mt-4 text-xl font-bold">Delete this variable?</h2>
        <p className="mt-2 text-sm leading-6 text-muted">This permanently removes <code className="rounded bg-surface px-1.5 py-0.5 font-mono font-bold text-ink">{variable.key}</code> from this app's saved variables.</p>
        {error && <p role="alert" className="mt-4 rounded-xl bg-danger/10 px-3 py-2.5 text-sm font-medium text-danger">{error}</p>}
        <div className="mt-6 flex justify-end gap-2">
          <Button type="button" onClick={onCancel} disabled={deleting}>Keep variable</Button>
          <Button type="button" variant="danger" disabled={deleting} icon={<Trash2 aria-hidden="true" size={15} />} onClick={() => void confirm()}>{deleting ? 'Deleting…' : 'Delete variable'}</Button>
        </div>
    </Modal>
  )
}


function ClearVariablesConfirmation({
  variableCount,
  profileName,
  onCancel,
  onConfirm,
}: {
  variableCount: number
  profileName: string
  onCancel: () => void
  onConfirm: () => Promise<void>
}) {
  const [error, setError] = useState('')
  const [clearing, setClearing] = useState(false)

  async function confirm() {
    setClearing(true)
    setError('')
    try {
      await onConfirm()
    } catch (reason) {
      setError(messageFromError(reason, 'Unable to clear saved variables.'))
      setClearing(false)
    }
  }

  return (
    <Modal role="alertdialog" labelledBy="clear-variables-title" onClose={onCancel} className="max-w-md">
      <div className="grid size-11 place-items-center rounded-2xl bg-danger/10 text-danger"><CircleAlert aria-hidden="true" size={22} /></div>
      <h2 id="clear-variables-title" className="mt-4 text-xl font-bold">{profileName === GENERAL_PROFILE ? 'Clear custom environment?' : `Clear ${profileName} variables?`}</h2>
      <p className="mt-2 text-sm leading-6 text-muted">{profileName === GENERAL_PROFILE ? <>This permanently removes all {variableCount} saved {variableCount === 1 ? 'variable' : 'variables'} from this app. Shell integration settings and startup files are not changed.</> : <>This removes all {variableCount} profile-local {variableCount === 1 ? 'variable' : 'variables'} from {profileName}. Inherited General values remain available.</>}</p>
      {error && <p role="alert" className="mt-4 rounded-xl bg-danger/10 px-3 py-2.5 text-sm font-medium text-danger">{error}</p>}
      <div className="mt-6 flex justify-end gap-2">
        <Button type="button" onClick={onCancel} disabled={clearing}>Cancel</Button>
        <Button type="button" variant="danger" disabled={clearing} icon={<Trash2 aria-hidden="true" size={15} />} onClick={() => void confirm()}>{clearing ? 'Clearing…' : profileName === GENERAL_PROFILE ? 'Clear custom env' : 'Clear profile variables'}</Button>
      </div>
    </Modal>
  )
}

function DeleteProfileConfirmation({
  profileName,
  variableCount,
  onCancel,
  onConfirm,
}: {
  profileName: string
  variableCount: number
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
      setError(messageFromError(reason, 'Unable to delete this profile.'))
      setDeleting(false)
    }
  }

  return (
    <Modal role="alertdialog" labelledBy="delete-profile-title" onClose={onCancel} className="max-w-md">
      <div className="grid size-11 place-items-center rounded-2xl bg-danger/10 text-danger"><CircleAlert aria-hidden="true" size={22} /></div>
      <h2 id="delete-profile-title" className="mt-4 text-xl font-bold">Delete {profileName} profile?</h2>
      <p className="mt-2 text-sm leading-6 text-muted">This permanently deletes the {profileName} profile and its {variableCount} profile-local {variableCount === 1 ? 'variable' : 'variables'}. Shared General variables are not deleted. If this profile is active, General will become active.</p>
      {error && <p role="alert" className="mt-4 rounded-xl bg-danger/10 px-3 py-2.5 text-sm font-medium text-danger">{error}</p>}
      <div className="mt-6 flex justify-end gap-2">
        <Button type="button" onClick={onCancel} disabled={deleting}>Cancel</Button>
        <Button type="button" variant="danger" disabled={deleting} icon={<Trash2 aria-hidden="true" size={15} />} onClick={() => void confirm()}>{deleting ? 'Deleting…' : 'Delete profile'}</Button>
      </div>
    </Modal>
  )
}

function ImportDialog({
  preview,
  profileName,
  onClose,
  onApply,
}: {
  preview: ImportPreview
  profileName: string
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
      onApply(await variableApi.applyImport(profileName, preview.token))
    } catch (reason) {
      setError(messageFromError(reason, 'Unable to apply this import.'))
      setApplying(false)
    }
  }

  return (
    <Modal labelledBy="import-title" onClose={close} className="max-w-2xl">
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-start gap-3">
            <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-surface text-ink"><FileText size={19} /></div>
            <div><p className="text-xs font-bold uppercase tracking-[0.16em] text-danger">Review before saving</p><h2 id="import-title" className="mt-1 text-xl font-bold">Import preview</h2></div>
          </div>
          <Button type="button" variant="quiet" className="size-9 min-h-9 px-0" aria-label="Close import preview" onClick={close}><X size={18} /></Button>
        </div>
        <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3">
          <div className="count-card"><span className="count-number">{preview.added.length}</span><span className="count-label">New keys</span></div>
          <div className="count-card"><span className="count-number">{preview.updated.length}</span><span className="count-label">Will update</span></div>
          <div className="count-card col-span-2 sm:col-span-1"><span className="count-number">{preview.invalidLines.length}</span><span className="count-label">Invalid lines</span></div>
        </div>
        {preview.added.length + preview.updated.length > 0 && (
          <div className="mt-5 max-h-40 overflow-auto rounded-xl border border-line bg-surface px-4 py-3 text-sm">
            {preview.added.map((key) => <p key={`new-${key}`} className="flex items-center gap-2 py-1"><span className="size-2 rounded-full bg-success" /> <code className="font-mono">{key}</code><span className="ml-auto text-xs text-muted">new</span></p>)}
            {preview.updated.map((key) => <p key={`updated-${key}`} className="flex items-center gap-2 py-1"><span className="size-2 rounded-full bg-secondary" /> <code className="font-mono">{key}</code><span className="ml-auto text-xs text-muted">update</span></p>)}
          </div>
        )}
        {preview.invalidLines.length > 0 && (
          <div className="mt-4 rounded-xl border border-danger/20 bg-danger/5 p-4">
            <p className="text-sm font-bold text-danger">Fix these lines before importing</p>
            <ul className="mt-2 space-y-1.5 text-sm text-muted">{preview.invalidLines.map((line) => <li key={line.line}>Line {line.line}: {line.message}</li>)}</ul>
            <p className="mt-3 text-xs leading-5 text-muted">No changes will be applied while any invalid line remains.</p>
          </div>
        )}
        {preview.invalidLines.length === 0 && preview.added.length + preview.updated.length === 0 && <p className="mt-5 rounded-xl bg-surface/60 p-4 text-sm text-muted">This file contains no variable changes to apply.</p>}
        {error && <p role="alert" className="mt-4 rounded-xl bg-danger/10 px-3 py-2.5 text-sm font-medium text-danger">{error}</p>}
        <div className="mt-6 flex justify-end gap-2 border-t border-line pt-4">
          <Button type="button" onClick={close}>Cancel</Button>
          <Button type="button" variant="primary" disabled={!canApply || applying} icon={<ArrowUpFromLine size={16} />} onClick={() => void apply()}>{applying ? 'Applying…' : 'Apply import'}</Button>
        </div>
    </Modal>
  )
}

export default function VariablesPage() {
  const [profiles, setProfiles] = useState<Profile[]>([])
  const [profilesLoading, setProfilesLoading] = useState(true)
  const [profilesError, setProfilesError] = useState('')
  const [selectedProfile, setSelectedProfile] = useState(GENERAL_PROFILE)
  const [activeProfile, setActiveProfile] = useState(GENERAL_PROFILE)
  const [profileToApply, setProfileToApply] = useState(GENERAL_PROFILE)
  const [applyingProfile, setApplyingProfile] = useState(false)
  const [creatingProfile, setCreatingProfile] = useState(false)
  const [variables, setVariables] = useState<Variable[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState('')
  const [search, setSearch] = useState('')
  const [editor, setEditor] = useState<Variable | null | undefined>(undefined)
  const [deleting, setDeleting] = useState<Variable | null>(null)
  const [confirmingClear, setConfirmingClear] = useState(false)
  const [profileToDelete, setProfileToDelete] = useState<string | null>(null)
  const profileActionsRef = useRef<HTMLDetailsElement>(null)
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

  async function refreshProfiles() {
    setProfilesLoading(true)
    setProfilesError('')
    try {
      const loadedProfiles = await profileApi.list()
      setProfiles(loadedProfiles)
      const applied = loadedProfiles.find((profile) => profile.active)?.name
      if (applied) {
        setActiveProfile(applied)
        setProfileToApply(applied)
      }
    } catch (error) {
      setProfilesError(messageFromError(error, 'Unable to load profiles.'))
    } finally {
      setProfilesLoading(false)
    }
  }

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
      setVariables(await variableApi.list(selectedProfile))
    } catch (error) {
      setLoadError(messageFromError(error, 'Unable to load saved variables.'))
    } finally {
      setLoading(false)
    }
  }, [selectedProfile])

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
    void profileApi.list()
      .then((loadedProfiles) => {
        if (cancelled) return
        setProfiles(loadedProfiles)
        const applied = loadedProfiles.find((profile) => profile.active)?.name ?? GENERAL_PROFILE
        setActiveProfile(applied)
        setProfileToApply(applied)
        setSelectedProfile(applied)
        setProfilesError('')
      })
      .catch((error: unknown) => {
        if (!cancelled) setProfilesError(messageFromError(error, 'Unable to load profiles.'))
      })
      .finally(() => {
        if (!cancelled) setProfilesLoading(false)
      })
    return () => { cancelled = true }
  }, [])

  useEffect(() => {
    let cancelled = false
    void variableApi.list(selectedProfile)
      .then((loadedVariables) => {
        if (!cancelled) setVariables(loadedVariables)
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoadError(messageFromError(error, 'Unable to load saved variables.'))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => { cancelled = true }
  }, [selectedProfile])

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
      const result = await variableApi.previewImport(selectedProfile)
      if (result) setPreview(result)
    } catch (error) {
      setActionError(messageFromError(error, 'Unable to open an environment file.'))
    } finally {
      setImporting(false)
    }
  }

  async function removeVariable() {
    if (!deleting) return
    await variableApi.remove(selectedProfile, deleting.key)
    setDeleting(null)
    await refresh()
  }

  async function clearSavedVariables() {
    await variableApi.clear(selectedProfile)
    setConfirmingClear(false)
    setSearch('')
    setNotice(selectedProfile === GENERAL_PROFILE ? 'Custom environment cleared from this app. Shell integration settings and startup files were left unchanged.' : `${selectedProfile} profile-local variables cleared. Inherited General values remain available.`)
    await refresh()
  }

  async function deleteProfile(profileName: string) {
    await profileApi.remove(profileName)
    const nextActiveProfile = activeProfile === profileName ? GENERAL_PROFILE : activeProfile
    const nextAppliedProfile = profileToApply === profileName ? nextActiveProfile : profileToApply
    setProfiles((current) => current.filter((profile) => profile.name !== profileName).map((profile) => ({ ...profile, active: profile.name === nextActiveProfile })))
    setSelectedProfile(GENERAL_PROFILE)
    setActiveProfile(nextActiveProfile)
    setProfileToApply(nextAppliedProfile)
    setProfileToDelete(null)
    setSearch('')
    setLoading(true)
    setNotice(`${profileName} profile and its profile-local variables were deleted.`)
  }

  function closeProfileActions() {
    if (profileActionsRef.current) profileActionsRef.current.open = false
  }

  function imported(result: ImportResult) {
    setPreview(null)
    setNotice(`Import complete: ${result.added.length} added, ${result.updated.length} updated.`)
    void refresh()
  }

  async function createdProfile(profile: Profile) {
    setCreatingProfile(false)
    setProfiles((current) => current.some((item) => item.name === profile.name) ? current : [...current, profile])
    setLoading(true)
    setLoadError('')
    setSelectedProfile(profile.name)
    setActiveProfile(profile.name)
    setProfileToApply(profile.name)
    await refreshProfiles()
  }

  const localVariableCount = selectedProfile === GENERAL_PROFILE ? variables.length : variables.filter((variable) => !variable.inherited).length
  const visibleProfiles = profiles.filter((profile) => profile.name !== GENERAL_PROFILE)
  const profileTabNames = [GENERAL_PROFILE, ...visibleProfiles.map((profile) => profile.name)]
  const selectedTabIndex = Math.max(0, profileTabNames.indexOf(selectedProfile))

  function selectProfile(name: string) {
    if (name === selectedProfile) return
    setLoading(true)
    setLoadError('')
    setActionError('')
    setSelectedProfile(name)
    setSearch('')
    setNotice('')
  }

  async function applyProfile() {
    setApplyingProfile(true)
    setActionError('')
    try {
      await profileApi.setActive(profileToApply)
      setActiveProfile(profileToApply)
      setSelectedProfile(profileToApply)
      setProfiles((current) => current.map((profile) => ({ ...profile, active: profile.name === profileToApply })))
      setSearch('')
      setNotice(`${profileToApply} profile applied for future shell sessions.`)
    } catch (error) {
      setActionError(messageFromError(error, 'Unable to apply this profile.'))
    } finally {
      setApplyingProfile(false)
    }
  }

  function handleProfileTabKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    let nextIndex: number | undefined
    if (event.key === 'ArrowRight') nextIndex = (index + 1) % profileTabNames.length
    if (event.key === 'ArrowLeft') nextIndex = (index - 1 + profileTabNames.length) % profileTabNames.length
    if (event.key === 'Home') nextIndex = 0
    if (event.key === 'End') nextIndex = profileTabNames.length - 1
    if (nextIndex === undefined) return
    event.preventDefault()
    selectProfile(profileTabNames[nextIndex])
    document.getElementById(`profile-tab-${nextIndex}`)?.focus()
  }

  return (
    <WorkspaceLayout>
      <section className="mb-9 grid gap-8 lg:grid-cols-[1fr_260px] lg:items-end">
        <div>
          <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-line bg-accent/20 px-3 py-1.5 text-xs font-semibold text-muted"><Sparkles aria-hidden="true" size={14} className="text-ink" /> Your environment, organized</div>
          <h1 className="max-w-2xl text-3xl font-bold leading-tight tracking-[-0.04em] sm:text-4xl">A calmer place for your <span className="text-danger">environment variables.</span></h1>
          <p className="mt-3 max-w-2xl text-sm leading-6 text-muted sm:text-base">Keep the values you use across projects in one private, user-scoped workspace.</p>
        </div>
        <div className="rounded-2xl border border-line bg-surface/80 p-4 shadow-card">
          <div className="flex items-start gap-3"><div className="mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg bg-surface text-ink"><Shield size={16} /></div><div><p className="text-sm font-bold">Saved here, not everywhere</p><p className="mt-1 text-xs leading-5 text-muted">Existing shells are unchanged. Enable Bash or Zsh integration below to load saved values in new terminal sessions.</p></div></div>
        </div>
      </section>

      <div className="mb-4 flex flex-wrap items-end gap-3">
        <Button type="button" variant="primary" onClick={() => setCreatingProfile(true)} icon={<Plus size={16} />}>Create profile</Button>
        {profilesLoading && <span role="status" className="text-xs text-muted">Loading profiles…</span>}
        {profilesError && <div role="alert" className="flex flex-1 items-center gap-2 rounded-xl bg-danger/5 px-3 py-2 text-sm text-danger"><CircleAlert size={16} />{profilesError}<Button type="button" className="ml-auto" onClick={() => void refreshProfiles()}>Retry profiles</Button></div>}
        <div className="ml-auto flex flex-wrap items-end gap-2">
          <label className="w-full sm:w-52">
            <span className="field-label">Profile to apply</span>
            <select className="field-input h-12" value={profileToApply} onChange={(event) => setProfileToApply(event.target.value)} disabled={profilesLoading}>
              <option value={GENERAL_PROFILE}>{GENERAL_PROFILE}</option>
              {visibleProfiles.map((profile) => <option key={profile.name} value={profile.name}>{profile.name}</option>)}
            </select>
          </label>
          <Button type="button" variant="primary" className="h-12" onClick={() => void applyProfile()} disabled={profilesLoading || Boolean(profilesError) || applyingProfile || profileToApply === activeProfile} icon={<ArrowUpFromLine size={16} />}>{applyingProfile ? 'Applying…' : 'Apply profile'}</Button>
        </div>
      </div>

      <section id="profile-panel" aria-label="Environment profiles" className="overflow-hidden rounded-3xl border border-line bg-surface shadow-card">
        <div role="tablist" aria-label="Profiles" className="flex flex-wrap items-end border-b border-line bg-canvas/35 px-5 pt-2 sm:px-6">
          <button id="profile-tab-0" type="button" role="tab" tabIndex={selectedProfile === GENERAL_PROFILE ? 0 : -1} aria-selected={selectedProfile === GENERAL_PROFILE} aria-controls="profile-content" onKeyDown={(event) => handleProfileTabKeyDown(event, 0)} onClick={() => selectProfile(GENERAL_PROFILE)} className={`-mb-px inline-flex min-h-11 items-center gap-2 border-b-2 px-4 py-2.5 text-sm transition-colors focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ink ${activeProfile === GENERAL_PROFILE ? 'border-b-4 font-extrabold text-ink' : selectedProfile === GENERAL_PROFILE ? 'border-secondary font-semibold text-ink' : 'border-transparent font-semibold text-muted hover:border-line hover:text-ink'}`} style={{ borderBottomColor: activeProfile === GENERAL_PROFILE ? profiles.find((profile) => profile.name === GENERAL_PROFILE)?.color : selectedProfile === GENERAL_PROFILE ? 'var(--color-secondary)' : undefined }} title={activeProfile === GENERAL_PROFILE ? 'Applied profile' : undefined}><span aria-hidden="true" className="size-2.5 rounded-full bg-primary" style={{ backgroundColor: profiles.find((profile) => profile.name === GENERAL_PROFILE)?.color }} />General{activeProfile === GENERAL_PROFILE && <span className="sr-only">, applied</span>}</button>
          {visibleProfiles.map((profile, index) => {
            const isApplied = activeProfile === profile.name
            const isSelected = selectedProfile === profile.name
            return <button id={`profile-tab-${index + 1}`} key={profile.name} type="button" role="tab" tabIndex={isSelected ? 0 : -1} aria-selected={isSelected} aria-controls="profile-content" onKeyDown={(event) => handleProfileTabKeyDown(event, index + 1)} onClick={() => selectProfile(profile.name)} className={`-mb-px inline-flex min-h-11 items-center gap-2 border-b-2 px-4 py-2.5 text-sm transition-colors focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ink ${isApplied ? 'border-b-4 font-extrabold text-ink' : isSelected ? 'border-secondary font-semibold text-ink' : 'border-transparent font-semibold text-muted hover:border-line hover:text-ink'}`} style={{ borderBottomColor: isApplied ? profile.color : isSelected ? 'var(--color-secondary)' : undefined }} title={isApplied ? 'Applied profile' : undefined}><span aria-hidden="true" className="size-2.5 rounded-full" style={{ backgroundColor: profile.color }} />{profile.name}{isApplied && <span className="sr-only">, applied</span>}</button>
          })}
        </div>
        {selectedProfile !== GENERAL_PROFILE && <p className="px-6 pt-3 text-xs text-muted">{profiles.find((profile) => profile.name === selectedProfile)?.description || 'Profile variables can override shared General values.'}</p>}

        <div id="profile-content" role="tabpanel" tabIndex={0} aria-labelledby={`profile-tab-${selectedTabIndex}`}>
          <div className="flex flex-col gap-4 border-b border-line px-5 py-5 sm:flex-row sm:items-center sm:justify-between sm:px-6">
          <div><div className="flex items-center gap-2"><h2 id="variables-heading" className="text-lg font-bold tracking-tight">{selectedProfile === GENERAL_PROFILE ? 'Your variables' : `${selectedProfile} variables`}</h2><span className="rounded-full bg-canvas px-2.5 py-1 text-xs font-bold text-muted">{variables.length}</span></div><p className="mt-1 text-sm text-muted">{selectedProfile === GENERAL_PROFILE ? 'Shared variables inherited by profiles unless overridden' : 'Effective values include inherited General variables and profile-local overrides'}</p></div>
          <div className="flex flex-wrap gap-2">
            <details ref={profileActionsRef} className="relative">
              <summary aria-label="Profile actions" className="inline-flex min-h-10 list-none cursor-pointer items-center gap-2 rounded-xl border border-line bg-surface px-4 py-2 text-sm font-semibold text-ink transition-colors hover:bg-canvas focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink [&::-webkit-details-marker]:hidden">
                Profile actions <ChevronDown aria-hidden="true" size={15} />
              </summary>
              <div className="absolute right-0 top-full z-30 mt-2 min-w-52 rounded-xl border border-line bg-surface p-1.5 shadow-card">
                <button type="button" className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm font-medium text-ink hover:bg-canvas disabled:cursor-not-allowed disabled:opacity-50" disabled={importing} onClick={() => { closeProfileActions(); void startImport() }}><ArrowDownToLine aria-hidden="true" size={15} />Import .env</button>
                <button type="button" className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm font-medium text-ink hover:bg-canvas disabled:cursor-not-allowed disabled:opacity-50" disabled={localVariableCount === 0} onClick={() => { closeProfileActions(); setConfirmingClear(true) }}><Trash2 aria-hidden="true" size={15} />Clear profile variables</button>
                {selectedProfile !== GENERAL_PROFILE && <button type="button" className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm font-medium text-danger hover:bg-danger/5" onClick={() => { closeProfileActions(); setProfileToDelete(selectedProfile) }}><Trash2 aria-hidden="true" size={15} />Delete profile</button>}
              </div>
            </details>
            <Button type="button" variant="primary" onClick={() => { setActionError(''); setEditor(null) }} icon={<Plus size={17} />}>Add variable</Button>
          </div>
        </div>
        {notice && <div role="status" className="flex items-center gap-2 border-b border-line bg-success/10 px-6 py-3 text-sm font-semibold text-ink"><Check className="text-success" size={16} />{notice}<button type="button" aria-label="Dismiss message" className="ml-auto rounded p-1 hover:bg-surface/70" onClick={() => setNotice('')}><X size={15} /></button></div>}
        {actionError && <div role="alert" className="flex items-start gap-2 border-b border-line bg-danger/5 px-6 py-3 text-sm text-danger"><CircleAlert size={16} className="mt-0.5 shrink-0" />{actionError}<button type="button" aria-label="Dismiss error" className="ml-auto rounded p-1 hover:bg-surface/70" onClick={() => setActionError('')}><X size={15} /></button></div>}
        <VariableCollection
          profileName={selectedProfile}
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
        </div>
      </section>

      <section aria-labelledby="shell-integration-heading" className="mt-6 overflow-hidden rounded-3xl border border-line bg-surface shadow-card">
        <div className="border-b border-line px-5 py-5 sm:px-6">
          <div className="flex items-start gap-3">
            <div className="mt-0.5 grid size-9 shrink-0 place-items-center rounded-xl bg-surface text-ink"><Terminal aria-hidden="true" size={18} /></div>
            <div>
              <h2 id="shell-integration-heading" className="text-lg font-bold tracking-tight">Shell integration</h2>
              <p className="mt-1 max-w-3xl text-sm leading-6 text-muted">Opt in separately for each shell to make your managed variables available in new terminal sessions, for example with <code className="rounded bg-canvas px-1 font-mono text-xs">echo $VAR_NAME</code>.</p>
            </div>
          </div>
          <p className="mt-4 rounded-xl bg-canvas/70 px-4 py-3 text-sm leading-6 text-muted">Enabling or disabling integration edits only this app's managed block in your user startup file—not other settings. It affects new shells only, and exported values are inherited by commands started from those shells. To load the change in an existing shell, source its file (for example, <code className="font-mono text-xs">source ~/.bashrc</code> or <code className="font-mono text-xs">source ~/.zshrc</code>).</p>
        </div>
        {shellIntegrationLoadError && <div role="alert" className="flex flex-wrap items-center gap-3 border-b border-line bg-danger/5 px-5 py-3 text-sm text-danger sm:px-6"><CircleAlert aria-hidden="true" size={16} />{shellIntegrationLoadError}<Button type="button" className="ml-auto" onClick={() => void refreshShellIntegrations()}>Retry shell status</Button></div>}
        {shellIntegrationActionError && <p role="alert" className="border-b border-line bg-danger/5 px-5 py-3 text-sm text-danger sm:px-6">{shellIntegrationActionError}</p>}
        {shellIntegrationNotice && <p role="status" className="border-b border-line bg-success/10 px-5 py-3 text-sm font-semibold text-ink sm:px-6">{shellIntegrationNotice}</p>}
        <div className="grid gap-3 p-4 sm:grid-cols-2 sm:p-5">
          {SHELL_INTEGRATIONS.map(({ shell, label, startupFile }) => {
            const status = shellIntegrationStatuses.find((integration) => integration.shell === shell)
            const busy = shellIntegrationLoading || shellIntegrationActionShell !== null
            return (
              <article key={shell} className="rounded-2xl border border-line bg-surface p-4">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h3 className="font-bold">{label}</h3>
                    <p className="mt-1 text-xs text-muted">Managed file: <code className="font-mono">{status?.startupFile ?? startupFile}</code></p>
                  </div>
                  <p className={`rounded-full px-2.5 py-1 text-xs font-bold ${status?.enabled ? 'bg-success/10 text-ink' : 'bg-canvas text-muted'}`}>
                    {shellIntegrationLoading ? 'Checking…' : status ? (status.enabled ? 'Enabled' : 'Disabled') : 'Unavailable'}
                  </p>
                </div>
                <Button
                  type="button"
                  className="mt-4 w-full"
                  variant={status?.enabled ? 'secondary' : 'primary'}
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
      <p className="mt-4 flex items-start gap-2 px-1 text-xs leading-5 text-muted"><Shield aria-hidden="true" size={14} className="mt-0.5 shrink-0" />Values are masked until you choose to reveal them. The app never runs the contents of imported files.</p>

      {editor !== undefined && <VariableEditor profileName={selectedProfile} variable={editor} variables={variables} onClose={() => setEditor(undefined)} onSaved={refresh} />}
      {creatingProfile && <ProfileEditor profiles={profiles} onClose={() => setCreatingProfile(false)} onCreated={createdProfile} />}
      {deleting && <DeleteConfirmation variable={deleting} onCancel={() => setDeleting(null)} onConfirm={removeVariable} />}
      {confirmingClear && <ClearVariablesConfirmation variableCount={localVariableCount} profileName={selectedProfile} onCancel={() => setConfirmingClear(false)} onConfirm={clearSavedVariables} />}
      {profileToDelete && <DeleteProfileConfirmation profileName={profileToDelete} variableCount={localVariableCount} onCancel={() => setProfileToDelete(null)} onConfirm={() => deleteProfile(profileToDelete)} />}
      {preview && <ImportDialog preview={preview} profileName={selectedProfile} onClose={() => setPreview(null)} onApply={imported} />}
    </WorkspaceLayout>
  )
}
