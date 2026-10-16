import { invoke } from '@tauri-apps/api/core'

type Props = {
  [key: string]: string | number;
};

/**
 * Optional parameters for the trackError function.
 */
export type TrackErrorOptions = {
  // Report the error as fatal (severity "fatal", kind "crash"). Defaults to false.
  fatal?: boolean;
};

export async function trackEvent(name: string, props?: Props): Promise<void> {
  await invoke<string>('plugin:aptabase|track_event', { name, props });
}

/**
 * Reports an error to Aptabase: its type, message and stack trace. It appears on the Errors page of the dashboard.
 * Needs the `aptabase:allow-track-error` permission.
 */
export async function trackError(error: unknown, options?: TrackErrorOptions): Promise<void> {
  const { name, message, stack } = normalizeError(error);
  await invoke('plugin:aptabase|track_error', {
    errorType: name,
    message: `${name}: ${message}`,
    stackTrace: stack ?? null,
    fatal: options?.fatal === true,
  });
}

// Anything can be thrown: a value that isn't an Error is reported by its string form.
function normalizeError(error: unknown): { name: string; message: string; stack?: string } {
  if (error instanceof Error) {
    return { name: error.name || 'Error', message: error.message, stack: error.stack };
  }
  try {
    return { name: 'Error', message: String(error) };
  } catch {
    return { name: 'Error', message: '[unserializable error]' };
  }
}
