<script setup lang="ts">
import {call} from '~/utils/api'
import {save as saveDialog} from '@tauri-apps/plugin-dialog'
import {writeText} from '@tauri-apps/plugin-clipboard-manager'
import type {Credential} from '~/composables/useCredentials'

const props = defineProps<{instanceId: string; bucket: string}>()
const {t} = useI18n()
const {credentials, load} = useCredentials()

const credentialId = ref('')
const format = ref<'laravel' | 'node' | 'cli'>('laravel')
const text = ref('')
const loading = ref(false)
const error = ref<{code?: string; message?: string; hint?: string} | null>(null)
const saved = ref('')
const copied = ref(false)
const existingPath = ref('')

const formats = [
    {label: 'Laravel (.env)', value: 'laravel'},
    {label: 'Node @aws-sdk/client-s3', value: 'node'},
    {label: 'AWS CLI', value: 'cli'},
]

const available = computed(() => credentials.value.filter((item) => item.bucket === props.bucket))

async function setup() {
    text.value = ''
    error.value = null
    saved.value = ''
    copied.value = false
    existingPath.value = ''
    await load(props.instanceId, props.bucket)
    if (!available.value.some((item: Credential) => item.id === credentialId.value)) {
        credentialId.value = available.value[0]?.id ?? ''
    }
}

watch(() => [props.instanceId, props.bucket], setup, {immediate: true})
watch(format, () => {
    text.value = ''
    saved.value = ''
})

function setError(cause: unknown) {
    error.value = cause as {code?: string; message?: string; hint?: string}
}

async function generate() {
    if (!credentialId.value) return
    loading.value = true
    error.value = null
    saved.value = ''
    try {
        text.value = await call<string>('generate_env', {
            instanceId: props.instanceId,
            credentialId: credentialId.value,
            format: format.value,
        })
    } catch (cause) {
        setError(cause)
    } finally {
        loading.value = false
    }
}

async function copy() {
    if (!text.value) return
    await writeText(text.value)
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
}

async function saveAs(allowOverwrite = false) {
    if (!credentialId.value) return
    const path =
        allowOverwrite && existingPath.value
            ? existingPath.value
            : await saveDialog({
                  defaultPath: '.env',
                  filters: [{name: 'Environment', extensions: ['env', 'txt']}],
              })
    if (!path) return
    loading.value = true
    error.value = null
    saved.value = ''
    try {
        saved.value = await call<string>('write_env_file', {
            instanceId: props.instanceId,
            credentialId: credentialId.value,
            format: format.value,
            path,
            allowOverwrite,
        })
        existingPath.value = ''
    } catch (cause) {
        const err = cause as {code?: string; message?: string; hint?: string}
        if (err.code === 'file_exists') {
            existingPath.value = path
        }
        setError(cause)
    } finally {
        loading.value = false
    }
}
</script>

<template>
    <div class="space-y-4">
        <UCard>
            <template #header>
                <div>
                    <h3 class="font-medium">{{ t('env.title') }}</h3>
                    <p class="text-sm text-muted">{{ t('env.hint') }}</p>
                </div>
            </template>

            <div class="space-y-4">
                <div class="grid gap-3 sm:grid-cols-2">
                    <UFormField :label="t('env.credential')">
                        <USelect
                            v-model="credentialId"
                            :items="available.map((item) => ({label: item.accessKey, value: item.id}))"
                            :placeholder="t('env.noCredential')"
                            class="w-full"
                        />
                    </UFormField>
                    <UFormField :label="t('env.format')">
                        <USelect v-model="format" :items="formats" class="w-full"/>
                    </UFormField>
                </div>

                <UAlert
                    v-if="!available.length"
                    color="warning"
                    variant="subtle"
                    icon="i-lucide-key-round"
                    :title="t('env.noCredential')"
                    :description="t('env.noCredentialHint')"
                />

                <UAlert
                    v-if="error"
                    color="error"
                    variant="subtle"
                    icon="i-lucide-circle-alert"
                    :title="error.message"
                    :description="error.hint"
                >
                    <template v-if="error.code === 'file_exists'" #actions>
                        <UButton
                            :label="t('env.overwrite')"
                            color="error"
                            size="sm"
                            :loading="loading"
                            @click="saveAs(true)"
                        />
                    </template>
                </UAlert>

                <UAlert
                    v-if="saved"
                    color="success"
                    variant="subtle"
                    icon="i-lucide-circle-check"
                    :title="t('env.saved', {path: saved})"
                />

                <div class="flex flex-wrap gap-2">
                    <UButton
                        icon="i-lucide-file-code"
                        :label="t('env.generate')"
                        :loading="loading"
                        :disabled="!credentialId"
                        @click="generate"
                    />
                    <UButton
                        v-if="text"
                        icon="i-lucide-copy"
                        :label="copied ? t('env.copied') : t('env.copy')"
                        color="neutral"
                        variant="subtle"
                        @click="copy"
                    />
                    <UButton
                        v-if="text"
                        icon="i-lucide-download"
                        :label="t('env.save')"
                        color="neutral"
                        variant="outline"
                        :loading="loading"
                        @click="saveAs()"
                    />
                </div>

                <div v-if="text">
                    <p class="mb-2 text-sm font-medium">{{ t('env.preview') }}</p>
                    <pre class="overflow-x-auto rounded bg-elevated p-3 text-xs">{{ text }}</pre>
                    <p class="mt-2 text-xs text-muted">{{ t('env.secretWarning') }}</p>
                </div>
            </div>
        </UCard>
    </div>
</template>
