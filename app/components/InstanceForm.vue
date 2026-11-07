<script setup lang="ts">
import type {Connection, Instance, InstanceTest, KeyLevel} from '~/composables/useInstances'

const props = defineProps<{instance?: Instance | null}>()
const open = defineModel<boolean>('open', {default: false})
const emit = defineEmits<{saved: [instance: Instance]}>()

const {t} = useI18n()
const {saveInstance, deleteInstance, testInstance} = useInstances()

const form = reactive({
    label: '',
    endpoint: '',
    region: 'us-east-1',
    pathStyle: true,
    tlsVerify: true,
    accessKey: '',
    secretKey: '',
})

const testing = ref(false)
const saving = ref(false)
const deleting = ref(false)
const confirmingDelete = ref(false)
const confirmation = ref('')
const result = ref<InstanceTest | null>(null)
const error = ref<{message?: string; hint?: string} | string | null>(null)

watch(open, value => {
    if (!value) return
    form.label = props.instance?.label ?? ''
    form.endpoint = props.instance?.endpoint ?? ''
    form.region = props.instance?.region ?? 'us-east-1'
    form.pathStyle = props.instance?.pathStyle ?? true
    form.tlsVerify = props.instance?.tlsVerify ?? true
    form.accessKey = ''
    form.secretKey = ''
    result.value = null
    error.value = null
    confirmingDelete.value = false
    confirmation.value = ''
})

const errorMessage = computed(() =>
    typeof error.value === 'string' ? error.value : (error.value?.message ?? ''),
)
const errorHint = computed(() =>
    typeof error.value === 'object' && error.value ? (error.value.hint ?? null) : null,
)

const qualification = computed(() => {
    const q = result.value?.qualification
    if (!q) return null
    const styles: Record<KeyLevel, {color: 'warning' | 'info' | 'error'; icon: string; key: string}> = {
        root: {color: 'warning', icon: 'i-lucide-shield-alert', key: 'levelRoot'},
        admin: {color: 'info', icon: 'i-lucide-shield-check', key: 'levelAdmin'},
        limited: {color: 'warning', icon: 'i-lucide-key-round', key: 'levelLimited'},
        invalid: {color: 'error', icon: 'i-lucide-shield-x', key: 'levelInvalid'},
    }
    return {...styles[q.level], text: t(`instanceForm.${styles[q.level].key}`)}
})

function connection(): Connection {
    return {
        endpoint: form.endpoint,
        region: form.region,
        pathStyle: form.pathStyle,
        tlsVerify: form.tlsVerify,
        accessKey: form.accessKey,
        secretKey: form.secretKey,
    }
}

async function runTest() {
    testing.value = true
    error.value = null
    try {
        result.value = await testInstance(connection())
    } catch (e) {
        result.value = null
        error.value = e as {message?: string; hint?: string}
    } finally {
        testing.value = false
    }
}

async function save() {
    saving.value = true
    error.value = null
    try {
        const lastCheck = result.value
            ? {at: new Date().toISOString(), ok: true, buckets: result.value.buckets.length}
            : (props.instance?.lastCheck ?? null)
        const saved = await saveInstance(
            {...form, id: props.instance?.id, lastCheck},
            form.accessKey,
            form.secretKey,
        )
        emit('saved', saved)
        open.value = false
    } catch (e) {
        error.value = e as {message?: string; hint?: string}
    } finally {
        saving.value = false
    }
}

async function remove() {
    if (!props.instance || confirmation.value !== props.instance.label) return
    deleting.value = true
    error.value = null
    try {
        await deleteInstance(props.instance.id)
        confirmingDelete.value = false
        open.value = false
    } catch (e) {
        error.value = e as {message?: string; hint?: string}
    } finally {
        deleting.value = false
    }
}
</script>

<template>
    <UModal
        v-model:open="open"
        :title="instance ? t('instanceForm.editTitle') : t('instanceForm.addTitle')"
    >
        <template #body>
            <div class="grid gap-4 sm:grid-cols-2">
                <UFormField :label="t('instanceForm.label')" name="label" required class="sm:col-span-2">
                    <UInput v-model="form.label" :placeholder="t('instanceForm.labelPlaceholder')" class="w-full"/>
                </UFormField>

                <UFormField :label="t('instanceForm.endpoint')" name="endpoint" required class="sm:col-span-2">
                    <UInput
                        v-model="form.endpoint"
                        :placeholder="t('instanceForm.endpointPlaceholder')"
                        class="w-full font-mono"
                    />
                </UFormField>

                <UFormField :label="t('instanceForm.region')" name="region">
                    <UInput v-model="form.region" class="w-full"/>
                </UFormField>

                <UFormField :label="t('instanceForm.accessKey')" name="accessKey" required>
                    <UInput
                        v-model="form.accessKey"
                        autocomplete="off"
                        class="w-full font-mono"
                        :placeholder="instance ? instance.adminAccessKeySuffix : undefined"
                    />
                </UFormField>

                <UFormField
                    :label="t('instanceForm.secretKey')"
                    name="secretKey"
                    :hint="instance ? t('instanceForm.secretKeep') : undefined"
                    required
                >
                    <UInput
                        v-model="form.secretKey"
                        type="password"
                        autocomplete="new-password"
                        class="w-full font-mono"
                    />
                </UFormField>

                <div class="flex flex-col gap-2 sm:col-span-2">
                    <UCheckbox v-model="form.pathStyle" :label="t('instanceForm.pathStyle')"/>
                    <UCheckbox v-model="form.tlsVerify" :label="t('instanceForm.tlsVerify')"/>
                </div>

                <div class="sm:col-span-2">
                    <UButton
                        :label="testing ? t('instanceForm.testing') : t('instanceForm.test')"
                        icon="i-lucide-plug-zap"
                        color="neutral"
                        variant="outline"
                        :loading="testing"
                        @click="runTest"
                    />
                </div>

                <div v-if="result" class="space-y-2 sm:col-span-2">
                    <UAlert
                        :color="qualification?.color"
                        :icon="qualification?.icon"
                        :title="qualification?.text"
                    />
                    <p class="text-sm text-muted">
                        {{ t('instanceForm.buckets', {n: result.buckets.length}) }}
                        <template v-if="result.location">
                            · {{ t('instanceForm.location', {region: result.location}) }}
                        </template>
                    </p>
                </div>

                <UAlert
                    v-if="errorMessage"
                    class="sm:col-span-2"
                    color="error"
                    icon="i-lucide-circle-alert"
                    :title="errorMessage"
                    :description="errorHint ?? undefined"
                />

                <div v-if="confirmingDelete" class="space-y-2 rounded-lg border border-error/40 p-3 sm:col-span-2">
                    <p class="text-sm">{{ t('instanceForm.deleteHint') }}</p>
                    <UInput
                        v-model="confirmation"
                        :placeholder="t('instanceForm.deleteConfirmLabel', {label: instance?.label})"
                        class="w-full"
                    />
                    <UButton
                        color="error"
                        :label="t('instanceForm.delete')"
                        :loading="deleting"
                        :disabled="confirmation !== instance?.label"
                        @click="remove"
                    />
                </div>
            </div>
        </template>

        <template #footer>
            <div class="flex w-full items-center gap-2">
                <UButton
                    v-if="instance && !confirmingDelete"
                    color="error"
                    variant="ghost"
                    :label="t('instanceForm.delete')"
                    @click="confirmingDelete = true"
                />
                <div class="ml-auto flex gap-2">
                    <UButton
                        color="neutral"
                        variant="ghost"
                        :label="t('instanceForm.cancel')"
                        @click="open = false"
                    />
                    <UButton :label="t('instanceForm.save')" :loading="saving" @click="save"/>
                </div>
            </div>
        </template>
    </UModal>
</template>
