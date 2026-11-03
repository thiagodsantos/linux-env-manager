import { invoke } from '@tauri-apps/api/core'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { shellIntegrationApi, variableApi } from '@/api'
import type { ImportPreview, ImportResult, ShellIntegrationStatus, Variable } from '@/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

beforeEach(() => vi.clearAllMocks())

describe('typed native API', () => {
  it('maps list and save calls to the Tauri command boundary', async () => {
    const variables: Variable[] = [{ key: 'HOME', value: '/home/test' }]
    vi.mocked(invoke).mockResolvedValueOnce(variables).mockResolvedValueOnce(undefined)
    await expect(variableApi.list()).resolves.toEqual(variables)
    await expect(variableApi.save({ key: 'HOME', value: '/tmp', originalKey: 'HOME' })).resolves.toBeUndefined()
    expect(invoke).toHaveBeenNthCalledWith(1, 'list_variables')
    expect(invoke).toHaveBeenNthCalledWith(2, 'save_variable', { input: { key: 'HOME', value: '/tmp', originalKey: 'HOME' } })
  })

  it('maps shell integration status and opt-in changes', async () => {
    const statuses: ShellIntegrationStatus[] = [
      { shell: 'bash', enabled: true, startupFile: '~/.bashrc' },
      { shell: 'zsh', enabled: false, startupFile: '~/.zshrc' },
    ]
    vi.mocked(invoke).mockResolvedValueOnce(statuses).mockResolvedValueOnce(undefined)
    await expect(shellIntegrationApi.getStatus()).resolves.toEqual(statuses)
    await expect(shellIntegrationApi.set({ shell: 'zsh', enabled: true })).resolves.toBeUndefined()
    expect(invoke).toHaveBeenNthCalledWith(1, 'get_shell_integrations')
    expect(invoke).toHaveBeenNthCalledWith(2, 'set_shell_integration', { shell: 'zsh', enabled: true })
  })

  it('maps delete, preview, and apply calls', async () => {
    const preview: ImportPreview = { token: 'token', added: ['NEW'], updated: [], invalidLines: [] }
    const result: ImportResult = { added: ['NEW'], updated: [] }
    vi.mocked(invoke).mockResolvedValueOnce(undefined).mockResolvedValueOnce(preview).mockResolvedValueOnce(result).mockResolvedValueOnce(undefined)
    await variableApi.remove('OLD')
    await expect(variableApi.previewImport()).resolves.toEqual(preview)
    await expect(variableApi.applyImport('token')).resolves.toEqual(result)
    await expect(variableApi.discardImport('token')).resolves.toBeUndefined()
    expect(invoke).toHaveBeenNthCalledWith(1, 'delete_variable', { key: 'OLD' })
    expect(invoke).toHaveBeenNthCalledWith(2, 'preview_import')
    expect(invoke).toHaveBeenNthCalledWith(3, 'apply_import', { token: 'token' })
    expect(invoke).toHaveBeenNthCalledWith(4, 'discard_import', { token: 'token' })
  })
})
