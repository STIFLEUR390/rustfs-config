import {invoke} from '@tauri-apps/api/core'

export type ConnectionStatus = 'ok' | 'error' | 'unknown'

export interface InstanceSummary {
    id: string
    label: string
    endpoint: string
    region: string
    pathStyle: boolean
    status: ConnectionStatus
    lastError?: string
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

// Fake data until instance CRUD lands (Kaneo #5).
const instances = ref<InstanceSummary[]>([
    {
        id: 'ins_local',
        label: 'Local (lerd)',
        endpoint: 'http://127.0.0.1:9000',
        region: 'us-east-1',
        pathStyle: true,
        status: 'ok',
    },
    {
        id: 'ins_prod',
        label: 'Prod Aplix',
        endpoint: 'https://storage.applix.fr',
        region: 'us-east-1',
        pathStyle: true,
        status: 'error',
        lastError: 'connect ECONNREFUSED 10.0.0.4:443',
    },
])
const activeInstanceId = ref<string | null>('ins_local')

export function useInstances() {
    const activeInstance = computed(
        () => instances.value.find(instance => instance.id === activeInstanceId.value) ?? null,
    )

    function selectInstance(id: string) {
        activeInstanceId.value = id
    }

    /** Backend round-trip; returns the bucket count on success. */
    function testConnection(connection: Connection) {
        return invoke<{ buckets: number }>('test_connection', {connection})
    }

    return {instances, activeInstanceId, activeInstance, selectInstance, testConnection}
}
