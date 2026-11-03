import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { shellIntegrationApi, variableApi } from '@/api'
import VariablesPage from '@/components/pages/VariablesPage'
import type { ImportPreview, ShellIntegrationStatus, Variable } from '@/types'

vi.mock('@/api', () => ({
  shellIntegrationApi: {
    getStatus: vi.fn(),
    set: vi.fn(),
  },
  variableApi: {
    list: vi.fn(),
    save: vi.fn(),
    remove: vi.fn(),
    clear: vi.fn(),
    previewImport: vi.fn(),
    applyImport: vi.fn(),
    discardImport: vi.fn(),
  },
}))

const savedVariables: Variable[] = [
  { key: 'API_TOKEN', value: 'secret-value' },
  { key: 'HOME', value: '/home/tester' },
]

const goodPreview: ImportPreview = {
  token: 'import-token',
  added: ['NEW_KEY'],
  updated: ['HOME'],
  invalidLines: [],
}

const disabledShells: ShellIntegrationStatus[] = [
  { shell: 'bash', enabled: false, startupFile: '~/.bashrc' },
  { shell: 'zsh', enabled: false, startupFile: '~/.zshrc' },
]

function mockInitialList(variables: Variable[] = savedVariables) {
  vi.mocked(variableApi.list).mockResolvedValue(variables)
}

async function openEditor(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(screen.getByRole('button', { name }))
  return screen.findByRole('dialog', { name: /variable/i })
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInitialList()
  vi.mocked(shellIntegrationApi.getStatus).mockResolvedValue(disabledShells)
  vi.mocked(shellIntegrationApi.set).mockResolvedValue(undefined)
  vi.mocked(variableApi.save).mockResolvedValue(undefined)
  vi.mocked(variableApi.remove).mockResolvedValue(undefined)
  vi.mocked(variableApi.clear).mockResolvedValue(undefined)
  vi.mocked(variableApi.previewImport).mockResolvedValue(null)
  vi.mocked(variableApi.applyImport).mockResolvedValue({ added: ['NEW_KEY'], updated: ['HOME'] })
  vi.mocked(variableApi.discardImport).mockResolvedValue(undefined)
})

describe('VariablesPage', () => {
  it('shows loading, masks values, and reveals then hides a value on request', async () => {
    let resolveList!: (values: Variable[]) => void
    vi.mocked(variableApi.list).mockReturnValue(new Promise((resolve) => { resolveList = resolve }))
    render(<VariablesPage />)
    expect(screen.getByRole('status')).toHaveTextContent('Loading')
    resolveList(savedVariables)
    expect(await screen.findByText('API_TOKEN')).toBeInTheDocument()
    expect(screen.getByLabelText('API_TOKEN value')).toHaveTextContent('••••••••••••')
    expect(screen.queryByText('secret-value')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Show API_TOKEN value' })).toHaveClass('variable-action')
    expect(screen.getByRole('button', { name: 'Edit API_TOKEN' })).toHaveClass('variable-action')
    expect(screen.getByRole('button', { name: 'Delete API_TOKEN' })).toHaveClass('variable-action')

    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Show API_TOKEN value' }))
    expect(screen.getByLabelText('API_TOKEN value')).toHaveTextContent('secret-value')
    await user.click(screen.getByRole('button', { name: 'Hide API_TOKEN value' }))
    expect(screen.getByLabelText('API_TOKEN value')).toHaveTextContent('••••••••••••')
  })

  it('validates keys inline and creates a new variable without overwriting duplicates', async () => {
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await openEditor(user, 'Add variable')

    await user.type(screen.getByLabelText('Key'), '9bad-key')
    await user.type(screen.getByLabelText('Value'), 'value')
    await user.click(screen.getByRole('button', { name: 'Save variable' }))
    expect(screen.getByRole('alert')).toHaveTextContent('Use a letter or underscore')
    expect(variableApi.save).not.toHaveBeenCalled()

    await user.clear(screen.getByLabelText('Key'))
    await user.type(screen.getByLabelText('Key'), 'HOME')
    await user.click(screen.getByRole('button', { name: 'Save variable' }))
    expect(screen.getByRole('alert')).toHaveTextContent('already exists')
    expect(variableApi.save).not.toHaveBeenCalled()

    await user.clear(screen.getByLabelText('Key'))
    await user.type(screen.getByLabelText('Key'), 'NEW_KEY')
    await user.click(screen.getByRole('button', { name: 'Save variable' }))
    await waitFor(() => expect(variableApi.save).toHaveBeenCalledWith({ key: 'NEW_KEY', value: 'value', originalKey: null }))
    expect(await screen.findByText('Your variables')).toBeInTheDocument()
  })

  it('updates a variable and reports backend save errors without closing the editor', async () => {
    vi.mocked(variableApi.save).mockRejectedValueOnce(new Error('Unable to save.'))
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await user.click(screen.getByRole('button', { name: 'Edit API_TOKEN' }))
    await user.clear(screen.getByLabelText('Value'))
    await user.type(screen.getByLabelText('Value'), 'updated-secret')
    await user.click(screen.getByRole('button', { name: 'Save variable' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Unable to save.')
    expect(screen.getByRole('dialog', { name: /edit variable/i })).toBeInTheDocument()
    expect(variableApi.save).toHaveBeenCalledWith({ key: 'API_TOKEN', value: 'updated-secret', originalKey: 'API_TOKEN' })

    vi.mocked(variableApi.save).mockResolvedValueOnce(undefined)
    await user.click(screen.getByRole('button', { name: 'Save variable' }))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: /edit variable/i })).not.toBeInTheDocument())
  })

  it('requires explicit delete confirmation and leaves data unchanged when cancelled', async () => {
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await user.click(screen.getByRole('button', { name: 'Delete API_TOKEN' }))
    const dialog = screen.getByRole('alertdialog', { name: 'Delete this variable?' })
    expect(within(dialog).getByText('API_TOKEN')).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Keep variable' }))
    expect(variableApi.remove).not.toHaveBeenCalled()
    expect(screen.getByText('API_TOKEN')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Delete API_TOKEN' }))
    await user.click(screen.getByRole('button', { name: 'Delete variable' }))
    await waitFor(() => expect(variableApi.remove).toHaveBeenCalledWith('API_TOKEN'))
    expect(await screen.findByText('Your variables')).toBeInTheDocument()
  })

  it('clears only the app-saved variables after confirmation', async () => {
    vi.mocked(variableApi.list).mockResolvedValueOnce(savedVariables).mockResolvedValueOnce([])
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')

    await user.click(screen.getByRole('button', { name: 'Clear custom env' }))
    const dialog = screen.getByRole('alertdialog', { name: 'Clear custom environment?' })
    expect(within(dialog).getByText(/all 2 saved variables from this app/i)).toBeInTheDocument()
    expect(within(dialog).getByText(/Shell integration settings and startup files are not changed/i)).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    expect(variableApi.clear).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'Clear custom env' }))
    const confirmation = screen.getByRole('alertdialog', { name: 'Clear custom environment?' })
    await user.click(within(confirmation).getByRole('button', { name: 'Clear custom env' }))
    await waitFor(() => expect(variableApi.clear).toHaveBeenCalledTimes(1))
    expect(await screen.findByText('A fresh start')).toBeInTheDocument()
    expect(shellIntegrationApi.set).not.toHaveBeenCalled()
  })

  it('keeps the clear confirmation open after a clear failure', async () => {
    vi.mocked(variableApi.clear).mockRejectedValueOnce(new Error('Clear failed'))
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await user.click(screen.getByRole('button', { name: 'Clear custom env' }))
    const dialog = screen.getByRole('alertdialog', { name: 'Clear custom environment?' })
    await user.click(within(dialog).getByRole('button', { name: 'Clear custom env' }))
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('Clear failed')
    expect(dialog).toBeInTheDocument()
  })

  it('keeps the confirmation open after a delete failure', async () => {
    vi.mocked(variableApi.remove).mockRejectedValueOnce(new Error('Delete failed'))
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await user.click(screen.getByRole('button', { name: 'Delete API_TOKEN' }))
    await user.click(screen.getByRole('button', { name: 'Delete variable' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Delete failed')
    const dialog = screen.getByRole('alertdialog')
    expect(dialog).toBeInTheDocument()
    expect(within(dialog).getByText('API_TOKEN')).toBeInTheDocument()
  })

  it('shows import counts, applies the merge, and reports the result', async () => {
    vi.mocked(variableApi.previewImport).mockResolvedValue(goodPreview)
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await user.click(screen.getByRole('button', { name: 'Import .env' }))
    const dialog = await screen.findByRole('dialog', { name: 'Import preview' })
    expect(dialog).toBeInTheDocument()
    expect(within(dialog).getByText('NEW_KEY')).toBeInTheDocument()
    expect(within(dialog).getByText('HOME', { selector: 'code' })).toBeInTheDocument()
    const closeButton = within(dialog).getByRole('button', { name: 'Close import preview' })
    expect(closeButton).toHaveFocus()
    await user.tab({ shift: true })
    expect(within(dialog).getByRole('button', { name: 'Apply import' })).toHaveFocus()
    await user.tab()
    expect(closeButton).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Apply import' }))
    await waitFor(() => expect(variableApi.applyImport).toHaveBeenCalledWith('import-token'))
    expect(await screen.findByRole('status')).toHaveTextContent('1 added, 1 updated')
  })

  it('blocks applying a preview that contains invalid lines and allows cancelling', async () => {
    vi.mocked(variableApi.previewImport).mockResolvedValue({
      token: 'blocked', added: ['NEW_KEY'], updated: [], invalidLines: [{ line: 4, message: 'Expected KEY=value.' }],
    })
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await user.click(screen.getByRole('button', { name: 'Import .env' }))
    const applyButton = await screen.findByRole('button', { name: 'Apply import' })
    expect(screen.getByText('Line 4: Expected KEY=value.')).toBeInTheDocument()
    expect(applyButton).toBeDisabled()
    await user.keyboard('{Escape}')
    expect(variableApi.discardImport).toHaveBeenCalledWith('blocked')
    expect(screen.queryByRole('dialog', { name: 'Import preview' })).not.toBeInTheDocument()
    expect(variableApi.applyImport).not.toHaveBeenCalled()
  })

  it('supports an empty import, empty list, and search with no matches', async () => {
    vi.mocked(variableApi.previewImport).mockResolvedValue({ token: 'empty', added: [], updated: [], invalidLines: [] })
    render(<VariablesPage />)
    const user = userEvent.setup()
    await screen.findByText('API_TOKEN')
    await user.click(screen.getByRole('button', { name: 'Import .env' }))
    expect(await screen.findByText('This file contains no variable changes to apply.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Apply import' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: 'Close import preview' }))
    expect(variableApi.discardImport).toHaveBeenCalledWith('empty')
    await user.type(screen.getByRole('textbox', { name: 'Search variable keys' }), 'does-not-exist')
    expect(screen.getByText('No keys match “does-not-exist”.')).toBeInTheDocument()

    vi.mocked(variableApi.list).mockResolvedValueOnce([])
    const emptyView = render(<VariablesPage />)
    expect(await emptyView.findByText('A fresh start')).toBeInTheDocument()
  })

  it('shows opt-in controls for both shells and refreshes status after enabling and disabling', async () => {
    const afterEnable: ShellIntegrationStatus[] = [
      { shell: 'bash', enabled: true, startupFile: '~/.bashrc' },
      { shell: 'zsh', enabled: true, startupFile: '~/.zshrc' },
    ]
    const afterDisable: ShellIntegrationStatus[] = [
      { shell: 'bash', enabled: true, startupFile: '~/.bashrc' },
      { shell: 'zsh', enabled: false, startupFile: '~/.zshrc' },
    ]
    vi.mocked(shellIntegrationApi.getStatus)
      .mockResolvedValueOnce(disabledShells)
      .mockResolvedValueOnce(afterEnable)
      .mockResolvedValueOnce(afterDisable)
    render(<VariablesPage />)
    const user = userEvent.setup()

    expect(await screen.findByRole('heading', { name: 'Shell integration' })).toBeInTheDocument()
    expect(screen.getByText(/edits only this app's managed block in your user startup file/i)).toBeInTheDocument()
    expect(screen.getByText(/It affects new shells only/i)).toBeInTheDocument()
    expect(screen.getByText('~/.bashrc')).toBeInTheDocument()
    expect(screen.getByText('~/.zshrc')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Enable Bash integration' }))
    await waitFor(() => expect(shellIntegrationApi.set).toHaveBeenCalledWith({ shell: 'bash', enabled: true }))
    expect(await screen.findByRole('button', { name: 'Disable Bash integration' })).toBeInTheDocument()
    expect(await screen.findByRole('status')).toHaveTextContent('Bash shell integration enabled')

    await user.click(screen.getByRole('button', { name: 'Disable Zsh integration' }))
    await waitFor(() => expect(shellIntegrationApi.set).toHaveBeenCalledWith({ shell: 'zsh', enabled: false }))
    expect(await screen.findByRole('button', { name: 'Enable Zsh integration' })).toBeInTheDocument()
    expect(await screen.findByRole('status')).toHaveTextContent('Zsh shell integration disabled')
    expect(shellIntegrationApi.getStatus).toHaveBeenCalledTimes(3)
    expect(screen.queryByText('secret-value')).not.toBeInTheDocument()
  })

  it('shows shell-status loading and recovers from status and action failures', async () => {
    let resolveStatus!: (statuses: ShellIntegrationStatus[]) => void
    vi.mocked(shellIntegrationApi.getStatus).mockReturnValueOnce(new Promise((resolve) => { resolveStatus = resolve }))
    render(<VariablesPage />)
    expect(screen.getAllByText('Checking…')).toHaveLength(2)
    resolveStatus(disabledShells)
    expect(await screen.findByRole('button', { name: 'Enable Bash integration' })).toBeInTheDocument()

    vi.mocked(shellIntegrationApi.set).mockRejectedValueOnce(new Error('Could not update shell file'))
    vi.mocked(shellIntegrationApi.getStatus).mockRejectedValueOnce(new Error('Status service unavailable'))
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Enable Bash integration' }))
    expect(await screen.findByText('Could not update shell file')).toBeInTheDocument()
    expect(await screen.findByText('Status service unavailable')).toBeInTheDocument()
    expect(shellIntegrationApi.set).toHaveBeenCalledWith({ shell: 'bash', enabled: true })

    vi.mocked(shellIntegrationApi.getStatus).mockResolvedValueOnce(disabledShells)
    await user.click(screen.getByRole('button', { name: 'Retry shell status' }))
    expect(await screen.findByRole('button', { name: 'Enable Bash integration' })).toBeInTheDocument()
  })

  it('shows recoverable load and file-picker errors and supports retry', async () => {
    vi.mocked(variableApi.list).mockRejectedValueOnce('Unable to access the desktop keyring.')
    vi.mocked(variableApi.previewImport).mockRejectedValueOnce(new Error('Picker unavailable'))
    render(<VariablesPage />)
    const user = userEvent.setup()
    expect(await screen.findByText('Unable to access the desktop keyring.')).toBeInTheDocument()
    vi.mocked(variableApi.list).mockResolvedValueOnce(savedVariables)
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByText('API_TOKEN')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Import .env' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Picker unavailable')
    await user.click(screen.getByRole('button', { name: 'Dismiss error' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})
