import { invoke } from '@tauri-apps/api'

export async function execute2(): Promise<string> {
  console.log('abc')
  return await invoke<string>('plugin:aptabase|execute')
}