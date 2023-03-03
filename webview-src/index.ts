import { invoke } from '@tauri-apps/api'

type Props = {
  [key: string]: string | number
}

export async function trackEvent(name: string, props?: Props): Promise<void> {
  await invoke<string>('plugin:aptabase|track_event', { name, props })
}