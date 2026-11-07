import {invoke} from '@tauri-apps/api/core'

export type ConnectionStatus = 'ok' | 'error' | 'unknown'
export type KeyLevel = 'root' | 'admin' | 'limited' | 'invalid'

/** Mirrors `src-tauri/src/store.rs::Instance` (never carries a secret). */
export interface Instance {
    id: string
    label: string
    endpoint: string
    region: string
    pathStyle: boolean
    tlsVerify: boolean
    consoleUrl?: string | null
    adminAccessKeySuffix: string
    lastCheck?: {at: string; ok: boolean; buckets: number} | null
}

/** Mirrors `src-tauri/src/config.rs::Connection`. */
export interface Connection {
    endpoint: string
    region: string
    pathStyle: boolean
    tlsVerify: boolean
    accessKey: string
    secretKey: string
}

export interface KeyQualification {
    level: KeyLevel
    policyName?: string | null
    detail?: string | null
}

export interface InstanceTest {
    buckets: string[]
    location?: string | null
    qualification: KeyQualification
}

export interface InstanceInput {
    id?: string
    label: string
    endpoint: string
    region: string
    pathStyle: boolean
    tlsVerify: boolean
    lastCheck?: Instance['lastCheck']
}

const instances = ref<Instance[]>([])
const activeInstanceId = ref<string | null>(null)
const loadError = ref<string | null>(null)
let loaded = false

export function useInstances() {
    const activeInstance = computed(
        () => instances.value.find(instance => instance.id === activeInstanceId.value) ?? null,
    )

    function selectInstance(id: string) {
        activeInstanceId.value = id
    }

    async function loadInstances() {
        try {
            instances.value = await invoke<Instance[]>('list_instances')
            loadError.value = null
        } catch (error) {
            instances.value = []
            loadError.value = String(error)
        }
        if (!instances.value.some(instance => instance.id === activeInstanceId.value)) {
            activeInstanceId.value = instances.value[0]?.id ?? null
        }
    }

    function ensureLoaded() {
        if (loaded) return Promise.resolve()
        loaded = true
        return loadInstances()
    }

    async function saveInstance(input: InstanceInput, accessKey?: string, secretKey?: string) {
        const instance = {
            id: input.id ?? '',
            label: input.label,
            endpoint: input.endpoint,
            region: input.region,
            pathStyle: input.pathStyle,
            tlsVerify: input.tlsVerify,
            consoleUrl: null,
            adminAccessKeySuffix: '',
            lastCheck: input.lastCheck ?? null,
        }
        const saved = await invoke<Instance>('save_instance', {
            instance,
            accessKey: accessKey || null,
            secretKey: secretKey || null,
        })
        await loadInstances()
        activeInstanceId.value = saved.id
        return saved
    }

    async function deleteInstance(id: string) {
        await invoke('delete_instance', {id})
        await loadInstances()
    }

    function testInstance(connection: Connection) {
        return invoke<InstanceTest>('test_instance', {connection})
    }

    return {
        instances,
        activeInstanceId,
        activeInstance,
        loadError,
        ensureLoaded,
        loadInstances,
        selectInstance,
        saveInstance,
        deleteInstance,
        testInstance,
    }
}

export function instanceStatus(instance: Instance): ConnectionStatus {
    if (!instance.lastCheck) return 'unknown'
    return instance.lastCheck.ok ? 'ok' : 'error'
}
