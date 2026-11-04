import { invoke } from '@tauri-apps/api/core'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { profileApi, shellIntegrationApi, variableApi } from '@/api'
import type { ImportPreview, ImportResult, Profile, ShellIntegrationStatus, Variable } from '@/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

beforeEach(() => vi.clearAllMocks())

describe('typed native API', () => {
  it('maps profile list and create calls to Tauri', async () => {
    const profiles: Profile[] = [{ name: 'Work', description: 'Work setup', color: '#05668d', active: true }]
    vi.mocked(invoke).mockResolvedValueOnce(profiles).mockResolvedValueOnce(undefined).mockResolvedValueOnce(undefined).mockResolvedValueOnce(undefined)
    await expect(profileApi.list()).resolves.toEqual(profiles)
    await expect(profileApi.create({ name: 'Work', description: 'Work setup', color: '#05668d', importFrom: 'General' })).resolves.toBeUndefined()
    await expect(profileApi.setActive('Work')).resolves.toBeUndefined()
    await expect(profileApi.remove('Work')).resolves.toBeUndefined()
    expect(invoke).toHaveBeenNthCalledWith(1, 'list_profiles')
    expect(invoke).toHaveBeenNthCalledWith(2, 'create_profile', { input: { name: 'Work', description: 'Work setup', color: '#05668d', importFrom: 'General' } })
    expect(invoke).toHaveBeenNthCalledWith(3, 'set_active_profile', { profileName: 'Work' })
    expect(invoke).toHaveBeenNthCalledWith(4, 'delete_profile', { profileName: 'Work' })
  })

  it('maps profile-scoped list, save, clear, and delete calls', async () => {
    const variables: Variable[] = [{ key: 'HOME', value: '/home/test', inherited: true }]
    vi.mocked(invoke).mockResolvedValueOnce(variables).mockResolvedValueOnce(undefined).mockResolvedValueOnce(undefined).mockResolvedValueOnce(undefined)
    await expect(variableApi.list('Work')).resolves.toEqual(variables)
    await expect(variableApi.save({ profileName: 'Work', key: 'HOME', value: '/tmp', originalKey: 'HOME' })).resolves.toBeUndefined()
    await expect(variableApi.remove('Work', 'OLD')).resolves.toBeUndefined()
    await expect(variableApi.clear('Work')).resolves.toBeUndefined()
    expect(invoke).toHaveBeenNthCalledWith(1, 'list_variables', { profileName: 'Work' })
    expect(invoke).toHaveBeenNthCalledWith(2, 'save_variable', { input: { profileName: 'Work', key: 'HOME', value: '/tmp', originalKey: 'HOME' } })
    expect(invoke).toHaveBeenNthCalledWith(3, 'delete_variable', { profileName: 'Work', key: 'OLD' })
    expect(invoke).toHaveBeenNthCalledWith(4, 'clear_variables', { profileName: 'Work' })
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

  it('maps profile-scoped preview and apply calls', async () => {
    const preview: ImportPreview = { token: 'token', added: ['NEW'], updated: [], invalidLines: [] }
    const result: ImportResult = { added: ['NEW'], updated: [] }
    vi.mocked(invoke).mockResolvedValueOnce(preview).mockResolvedValueOnce(result).mockResolvedValueOnce(undefined)
    await expect(variableApi.previewImport('Work')).resolves.toEqual(preview)
    await expect(variableApi.applyImport('Work', 'token')).resolves.toEqual(result)
    await expect(variableApi.discardImport('token')).resolves.toBeUndefined()
    expect(invoke).toHaveBeenNthCalledWith(1, 'preview_import', { profileName: 'Work' })
    expect(invoke).toHaveBeenNthCalledWith(2, 'apply_import', { profileName: 'Work', token: 'token' })
    expect(invoke).toHaveBeenNthCalledWith(3, 'discard_import', { token: 'token' })
  })
})
