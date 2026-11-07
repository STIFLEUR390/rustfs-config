import {invoke} from '@tauri-apps/api/core'

export interface Credential {
    id: string
    instanceId: string
    bucket: string
    accessKey: string
    policyName: string
    mode: string
    createdAt: string
    rotatedAt: string | null
}

export interface IssuedCredential {
    credential: Credential
    secret: string | null
}

export type CredentialMode = 'rw' | 'ro' | 'rw-sans-suppression'

const credentials = ref<Credential[]>([])
const loading = ref(false)
const error = ref<{message?: string; hint?: string} | null>(null)

export function useCredentials() {
    async function load(instanceId: string, bucket: string) {
        loading.value = true
        error.value = null
        try {
            credentials.value = await invoke<Credential[]>('list_credentials', {instanceId, bucket})
        } catch (cause) {
            error.value = cause as {message?: string; hint?: string}
        } finally {
            loading.value = false
        }
    }

    async function create(instanceId: string, bucket: string, mode: CredentialMode, accessKey?: string) {
        const issued = await invoke<IssuedCredential>('create_credential', {
            instanceId,
            bucket,
            mode,
            accessKey: accessKey?.trim() || null,
        })
        await load(instanceId, bucket)
        return issued
    }

    async function adopt(instanceId: string, bucket: string, accessKey: string) {
        const issued = await invoke<IssuedCredential>('adopt_credential', {instanceId, bucket, accessKey})
        await load(instanceId, bucket)
        return issued
    }

    async function rotate(instanceId: string, credential: Credential) {
        const secret = await invoke<string>('rotate_credential', {
            instanceId,
            credentialId: credential.id,
        })
        await load(instanceId, credential.bucket)
        return secret
    }

    async function revoke(instanceId: string, credential: Credential) {
        await invoke('revoke_credential', {instanceId, credentialId: credential.id})
        await load(instanceId, credential.bucket)
    }

    return {credentials, loading, error, load, create, adopt, rotate, revoke}
}
