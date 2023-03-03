import { invoke } from '@tauri-apps/api'

export async function trackEvent(name: string): Promise<void> {
  await invoke<string>('plugin:aptabase|track_event', { name })
}