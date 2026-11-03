export interface Variable {
  key: string
  value: string
}

export interface SaveVariableInput {
  key: string
  value: string
  originalKey: string | null
}

export interface InvalidImportLine {
  line: number
  message: string
}

export interface ImportPreview {
  token: string
  added: string[]
  updated: string[]
  invalidLines: InvalidImportLine[]
}

export interface ImportResult {
  added: string[]
  updated: string[]
}

export type ShellIntegrationShell = 'bash' | 'zsh'

export interface ShellIntegrationStatus {
  shell: ShellIntegrationShell
  enabled: boolean
  startupFile: '~/.bashrc' | '~/.zshrc'
}

export interface SetShellIntegrationInput {
  shell: ShellIntegrationShell
  enabled: boolean
}
