import {invoke} from '@tauri-apps/api/core'
import type {Connection} from './useInstances'

export interface BucketSummary {
    name: string
    publicPrefixes: string[]
    privatePrefixes: string[]
    lifecycleDays: number | null
}

export interface ObjectEntry {
    key: string
    size: number
    lastModified: string | null
}

// Fake data until the explorer lands (Kaneo #6).
const bucketsByInstance = ref<Record<string, BucketSummary[]>>({
    ins_local: [
        {
            name: 'kwiktalk',
            publicPrefixes: ['uploads/media/'],
            privatePrefixes: ['whatsapp-media/'],
            lifecycleDays: 180,
        },
        {name: 'backups', publicPrefixes: [], privatePrefixes: [], lifecycleDays: null},
    ],
    ins_prod: [
        {
            name: 'kwiktalk',
            publicPrefixes: ['uploads/media/'],
            privatePrefixes: ['whatsapp-media/'],
            lifecycleDays: 180,
        },
    ],
})
const activeBucketName = ref<string | null>('kwiktalk')

export function useBucket() {
    function buckets(instanceId: string) {
        return bucketsByInstance.value[instanceId] ?? []
    }

    function selectBucket(name: string | null) {
        activeBucketName.value = name
    }

    function listObjects(connection: Connection, bucket: string, prefix = '') {
        return invoke<ObjectEntry[]>('list_objects', {connection, bucket, prefix})
    }

    return {bucketsByInstance, activeBucketName, buckets, selectBucket, listObjects}
}
