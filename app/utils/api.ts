import {invoke} from '@tauri-apps/api/core'
import * as Sentry from '@sentry/vue'

/**
 * Calls a Tauri command and reports backend failures to Sentry before rethrowing,
 * so Rust errors show up alongside frontend ones with their `code` as a tag.
 */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
        return await invoke<T>(command, args)
    } catch (error) {
        const {code, message} = (error ?? {}) as {code?: string; message?: string}
        Sentry.captureException(
            new Error(`${command}: ${code ?? 'error'} — ${message ?? String(error)}`),
            {tags: {command, ...(code ? {code} : {})}},
        )
        throw error
    }
}
