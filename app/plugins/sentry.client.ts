import * as Sentry from '@sentry/vue'
import {getVersion} from '@tauri-apps/api/app'

// Frontend errors land in the same project as the Rust SDK (src-tauri/src/lib.rs).
export default async () => {
    let release: string | undefined
    try {
        release = `rustfs-config@${await getVersion()}`
    } catch {
        // Plain browser (`bun run dev`): no Tauri IPC, keep the default release.
    }

    Sentry.init({
        dsn: 'https://af069846a5284527a6a2ffbcf327a2c4@rustrak-api.applix.fr/6',
        release,
        environment: import.meta.dev ? 'development' : 'production',
        // No PII and no performance tracing: only crash reports.
        sendDefaultPii: false,
        tracesSampleRate: 0,
    })
}
