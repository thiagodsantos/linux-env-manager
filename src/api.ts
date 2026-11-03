import { invoke } from '@tauri-apps/api/core'
import type { ImportPreview, ImportResult, SaveVariableInput, SetShellIntegrationInput, ShellIntegrationStatus, Variable } from '@/types'

export const variableApi = {
  list: () => invoke<Variable[]>('list_variables'),
  save: (input: SaveVariableInput) => invoke<void>('save_variable', { input }),
  remove: (key: string) => invoke<void>('delete_variable', { key }),
  previewImport: () => invoke<ImportPreview | null>('preview_import'),
  applyImport: (token: string) => invoke<ImportResult>('apply_import', { token }),
  discardImport: (token: string) => invoke<void>('discard_import', { token }),
}

export const shellIntegrationApi = {
  getStatus: () => invoke<ShellIntegrationStatus[]>('get_shell_integrations'),
  set: (input: SetShellIntegrationInput) => invoke<void>('set_shell_integration', { shell: input.shell, enabled: input.enabled }),
}
