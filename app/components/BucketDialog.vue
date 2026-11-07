<script setup lang="ts">
const props = defineProps<{
    mode: 'create' | 'delete'
    instanceId: string
    bucket?: string | null
}>()
const open = defineModel<boolean>('open', {default: false})
const emit = defineEmits<{done: []}>()

const {t} = useI18n()
const {createBucket, deleteBucket} = useBucket()

const name = ref('')
const busy = ref(false)
const error = ref<{message?: string; hint?: string} | null>(null)

watch(open, value => {
    if (!value) return
    name.value = ''
    error.value = null
})

async function submit() {
    busy.value = true
    error.value = null
    try {
        if (props.mode === 'create') {
            await createBucket(props.instanceId, name.value.trim())
        } else if (props.bucket) {
            await deleteBucket(props.instanceId, props.bucket)
        }
        open.value = false
        emit('done')
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}
</script>

<template>
    <UModal
        v-model:open="open"
        :title="mode === 'create' ? t('buckets.createTitle') : t('buckets.deleteTitle')"
    >
        <template #body>
            <div class="space-y-3">
                <p v-if="mode === 'delete'" class="text-sm">
                    {{ t('buckets.deleteHint', {name: bucket}) }}
                </p>
                <UFormField
                    :label="mode === 'create' ? t('buckets.name') : t('buckets.confirmLabel', {name: bucket})"
                    name="bucket"
                >
                    <UInput v-model="name" class="w-full font-mono" autocomplete="off"/>
                </UFormField>
                <UAlert
                    v-if="error?.message"
                    color="error"
                    icon="i-lucide-circle-alert"
                    :title="error.message"
                    :description="error.hint"
                />
            </div>
        </template>

        <template #footer>
            <div class="ml-auto flex gap-2">
                <UButton
                    color="neutral"
                    variant="ghost"
                    :label="t('instanceForm.cancel')"
                    @click="open = false"
                />
                <UButton
                    :color="mode === 'delete' ? 'error' : 'primary'"
                    :label="mode === 'create' ? t('buckets.create') : t('instanceForm.delete')"
                    :loading="busy"
                    :disabled="mode === 'delete' ? name !== bucket : !name.trim()"
                    @click="submit"
                />
            </div>
        </template>
    </UModal>
</template>
