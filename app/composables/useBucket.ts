import {invoke} from '@tauri-apps/api/core'

export interface ObjectEntry {
    key: string
    size: number
    lastModified: string | null
}

export interface ObjectPage {
    entries: ObjectEntry[]
    prefixes: string[]
    nextToken: string | null
}

export interface PolicyDiff {
    sid: string
    action: 'add' | 'update' | 'remove'
    before: unknown | null
    after: unknown | null
}

export interface PolicyPlan {
    bucket: string
    publicPrefixes: string[]
    document: unknown
    diff: PolicyDiff[]
    changed: boolean
    requiresConfirmation: boolean
}

const bucketNames = ref<Record<string, string[]>>({})
const activeBucketName = ref<string | null>(null)
const activePrefix = ref('')
const page = ref<ObjectPage>({entries: [], prefixes: [], nextToken: null})
const loading = ref(false)
const error = ref<unknown>(null)

export function useBucket() {
    function buckets(instanceId: string) {
        return bucketNames.value[instanceId] ?? []
    }

    async function loadBuckets(instanceId: string) {
        const names = await invoke<string[]>('list_instance_buckets', {instanceId})
        bucketNames.value = {...bucketNames.value, [instanceId]: names}
        if (!names.includes(activeBucketName.value ?? '')) {
            selectBucket(names[0] ?? null)
        }
    }

    function selectBucket(name: string | null) {
        activeBucketName.value = name
        activePrefix.value = ''
        page.value = {entries: [], prefixes: [], nextToken: null}
    }

    function openPrefix(prefix: string) {
        activePrefix.value = prefix
        page.value = {entries: [], prefixes: [], nextToken: null}
    }

    async function loadObjects(instanceId: string, token?: string) {
        const bucket = activeBucketName.value
        if (!bucket) {
            page.value = {entries: [], prefixes: [], nextToken: null}
            return
        }
        loading.value = true
        error.value = null
        try {
            const next = await invoke<ObjectPage>('list_instance_objects', {
                instanceId,
                bucket,
                prefix: activePrefix.value || null,
                token: token ?? null,
                maxKeys: 100,
            })
            page.value = token
                ? {
                    entries: [...page.value.entries, ...next.entries],
                    prefixes: page.value.prefixes,
                    nextToken: next.nextToken,
                }
                : next
        } catch (cause) {
            error.value = cause
            if (!token) page.value = {entries: [], prefixes: [], nextToken: null}
        } finally {
            loading.value = false
        }
    }

    async function createBucket(instanceId: string, name: string) {
        await invoke('create_instance_bucket', {instanceId, bucket: name})
        await loadBuckets(instanceId)
        selectBucket(name)
    }

    async function deleteBucket(instanceId: string, name: string) {
        await invoke('delete_instance_bucket', {instanceId, bucket: name})
        await loadBuckets(instanceId)
    }

    async function upload(instanceId: string, path: string, key?: string) {
        const bucket = activeBucketName.value
        if (!bucket) return
        const name = key ?? `${activePrefix.value}${path.split(/[\\/]/).pop() ?? path}`
        await invoke('upload_instance_object', {instanceId, bucket, key: name, path})
        await loadObjects(instanceId)
    }

    function download(instanceId: string, key: string, path: string) {
        return invoke('download_instance_object', {
            instanceId,
            bucket: activeBucketName.value,
            key,
            path,
        })
    }

    async function deleteObject(instanceId: string, key: string) {
        await invoke('delete_instance_object', {
            instanceId,
            bucket: activeBucketName.value,
            key,
        })
        await loadObjects(instanceId)
    }

    function bucketState(instanceId: string, bucket: string) {
        return invoke<{publicPrefixes: string[]; managed: {bucketPolicy: boolean}} | null>(
            'bucket_state',
            {instanceId, bucket},
        )
    }

    function planPolicy(instanceId: string, bucket: string, prefixes: string[]) {
        return invoke<PolicyPlan>('plan_bucket_policy', {instanceId, bucket, prefixes})
    }

    function applyPolicy(
        instanceId: string,
        bucket: string,
        prefixes: string[],
        allowRoot = false,
    ) {
        return invoke<PolicyPlan>('apply_bucket_policy', {
            instanceId,
            bucket,
            prefixes,
            allowRoot,
        })
    }

    return {
        bucketNames,
        activeBucketName,
        activePrefix,
        page,
        loading,
        error,
        buckets,
        loadBuckets,
        selectBucket,
        openPrefix,
        loadObjects,
        createBucket,
        deleteBucket,
        upload,
        download,
        deleteObject,
        bucketState,
        planPolicy,
        applyPolicy,
    }
}
