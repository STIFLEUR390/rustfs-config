import {check, type Update} from '@tauri-apps/plugin-updater'
import {relaunch} from '@tauri-apps/plugin-process'

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'current' | 'error'

const STORAGE_KEY = 'rc.updates.autoCheck'

const enabled = ref(false)
const status = ref<UpdateStatus>('idle')
const available = ref<{ version: string; notes?: string } | null>(null)
const error = ref<string | null>(null)
let pending: Update | null = null

if (import.meta.client) {
    enabled.value = localStorage.getItem(STORAGE_KEY) === '1'
}

function setEnabled(value: boolean) {
    enabled.value = value
    if (import.meta.client) {
        localStorage.setItem(STORAGE_KEY, value ? '1' : '0')
    }
}

async function checkForUpdates() {
    status.value = 'checking'
    error.value = null
    available.value = null
    pending = null
    try {
        const update = await check()
        if (update) {
            pending = update
            available.value = {version: update.version, notes: update.body ?? undefined}
            status.value = 'available'
        } else {
            status.value = 'current'
        }
    } catch (cause) {
        error.value = cause instanceof Error ? cause.message : String(cause)
        status.value = 'error'
    }
}

async function installUpdate() {
    if (!pending) return
    status.value = 'checking'
    error.value = null
    try {
        await pending.downloadAndInstall()
        await relaunch()
    } catch (cause) {
        error.value = cause instanceof Error ? cause.message : String(cause)
        status.value = 'error'
    }
}

/** Called at startup: does nothing unless the user opted in from the settings dialog. */
async function autoCheck() {
    if (enabled.value) await checkForUpdates()
}

export function useUpdater() {
    return {
        enabled,
        status,
        available,
        error,
        setEnabled,
        checkForUpdates,
        installUpdate,
        autoCheck,
    }
}
