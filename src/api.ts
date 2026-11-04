import { invoke } from '@tauri-apps/api/core'
import type { CreateProfileInput, ImportPreview, ImportResult, Profile, SaveVariableInput, SetShellIntegrationInput, ShellIntegrationStatus, Variable } from '@/types'

export const profileApi = {
  list: () => invoke<Profile[]>('list_profiles'),
  create: (input: CreateProfileInput) => invoke<void>('create_profile', { input }),
  setActive: (profileName: string) => invoke<void>('set_active_profile', { profileName }),
  remove: (profileName: string) => invoke<void>('delete_profile', { profileName }),
}

export const variableApi = {
  list: (profileName: string) => invoke<Variable[]>('list_variables', { profileName }),
  save: (input: SaveVariableInput) => invoke<void>('save_variable', { input }),
  remove: (profileName: string, key: string) => invoke<void>('delete_variable', { profileName, key }),
  clear: (profileName: string) => invoke<void>('clear_variables', { profileName }),
  previewImport: (profileName: string) => invoke<ImportPreview | null>('preview_import', { profileName }),
  applyImport: (profileName: string, token: string) => invoke<ImportResult>('apply_import', { profileName, token }),
  discardImport: (token: string) => invoke<void>('discard_import', { token }),
}

export const shellIntegrationApi = {
  getStatus: () => invoke<ShellIntegrationStatus[]>('get_shell_integrations'),
  set: (input: SetShellIntegrationInput) => invoke<void>('set_shell_integration', { shell: input.shell, enabled: input.enabled }),
}
