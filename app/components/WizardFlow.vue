<script setup lang="ts">
import {invoke} from '@tauri-apps/api/core'
import type {CredentialMode} from '~/composables/useCredentials'

const props = defineProps<{instanceId: string | null}>()
const open = defineModel<boolean>('open', {default: false})
const emit = defineEmits<{done: []}>()
const {t} = useI18n()
const {buckets, loadBuckets, createBucket, planPolicy, applyPolicy, planBucketConfig, applyBucketConfig} =
    useBucket()
const {create} = useCredentials()

type Template = 'laravel-media' | 'blank'

const steps = ['bucket', 'visibility', 'credentials', 'retention', 'preview'] as const
const step = ref(0)
const mode = ref<'new' | 'existing'>('new')
const name = ref('')
const template = ref<Template>('laravel-media')
const prefixes = ref<string[]>([])
const newPrefix = ref('')
const credentialMode = ref<CredentialMode>('rw')
const accessKey = ref('')
const rules = ref<{prefix: string; expireDays: number}[]>([])
const rulePrefix = ref('')
const ruleDays = ref('180')

const working = ref(false)
const error = ref<{code?: string; message?: string; hint?: string} | null>(null)
const previewPolicy = ref<{changed: boolean; diff: {sid: string; action: string}[]} | null>(null)
const previewConfig = ref<{rulesChanged: boolean; ruleDiff: {id: string; action: string}[]} | null>(
    null,
)
const results = ref<string[]>([])
const envText = ref('')

const current = computed(() => steps[step.value])
const alias = computed(() => props.instanceId ?? 'local')

function applyTemplate(value: Template) {
    if (value === 'laravel-media') {
        prefixes.value = ['uploads/media/']
        rules.value = [{prefix: 'uploads/media/', expireDays: 180}]
    } else {
        prefixes.value = []
        rules.value = []
    }
}

function reset() {
    step.value = 0
    mode.value = 'new'
    name.value = ''
    template.value = 'laravel-media'
    applyTemplate('laravel-media')
    credentialMode.value = 'rw'
    accessKey.value = ''
    newPrefix.value = ''
    rulePrefix.value = ''
    ruleDays.value = '180'
    previewPolicy.value = null
    previewConfig.value = null
    results.value = []
    envText.value = ''
    error.value = null
    if (props.instanceId) loadBuckets(props.instanceId).catch(() => {})
}

watch(open, (value) => {
    if (value) reset()
})
watch(template, applyTemplate)

function addPrefix() {
    const value = newPrefix.value.trim()
    if (!value) return
    const normalized = value.endsWith('/') ? value : `${value}/`
    if (!prefixes.value.includes(normalized)) prefixes.value = [...prefixes.value, normalized]
    newPrefix.value = ''
}

function addRule() {
    const days = Number(ruleDays.value)
    if (!Number.isInteger(days) || days <= 0) return
    const value = rulePrefix.value.trim()
    if (rules.value.some((rule) => rule.prefix === value)) return
    rules.value = [...rules.value, {prefix: value, expireDays: days}]
    rulePrefix.value = ''
}

function canContinue() {
    if (current.value === 'bucket') return name.value.trim().length > 0
    return true
}

async function preview() {
    if (!props.instanceId) return
    working.value = true
    error.value = null
    try {
        previewPolicy.value = prefixes.value.length
            ? await planPolicy(props.instanceId, name.value.trim(), prefixes.value)
            : null
        previewConfig.value = await planBucketConfig(
            props.instanceId,
            name.value.trim(),
            rules.value,
            null,
            null,
        )
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    } finally {
        working.value = false
    }
}

async function next() {
    if (!canContinue()) return
    if (current.value === 'retention') {
        await preview()
    }
    step.value = Math.min(step.value + 1, steps.length - 1)
}

function back() {
    step.value = Math.max(step.value - 1, 0)
}

function setError(cause: unknown) {
    error.value = cause as {code?: string; message?: string; hint?: string}
}

const rcCommands = computed(() => {
    const bucket = name.value.trim() || '<bucket>'
    const key = accessKey.value.trim() || `${bucket}-app`
    return [
        `rc bucket create ${alias.value} ${bucket}`,
        `rc bucket anonymous set-json ${alias.value} ${bucket} bucket-policy.json`,
        `rc admin user add ${alias.value} ${key} <secret>`,
        `rc admin policy create ${alias.value} rc-${bucket}-${credentialMode.value} policy.json`,
        `rc admin policy attach ${alias.value} rc-${bucket}-${credentialMode.value} --user ${key}`,
    ].join('\n')
})

async function run() {
    if (!props.instanceId) return
    const bucket = name.value.trim()
    if (!bucket) return
    working.value = true
    error.value = null
    results.value = []
    envText.value = ''
    try {
        if (mode.value === 'new' && !buckets(props.instanceId).includes(bucket)) {
            await createBucket(props.instanceId, bucket)
            results.value.push(t('wizard.resultBucket', {bucket}))
        }
        if (prefixes.value.length) {
            const applied = await applyPolicy(props.instanceId, bucket, prefixes.value, false)
            results.value.push(t('wizard.resultPolicy', {n: applied.diff.length}))
        }
        const issued = await create(
            props.instanceId,
            bucket,
            credentialMode.value,
            accessKey.value.trim() || undefined,
        )
        results.value.push(t('wizard.resultUser', {name: issued.credential.accessKey}))
        if (rules.value.length) {
            const applied = await applyBucketConfig(
                props.instanceId,
                bucket,
                rules.value,
                null,
                null,
                false,
            )
            results.value.push(t('wizard.resultLifecycle', {n: applied.ruleDiff.length}))
        }
        emit('done')
        if (issued.secret) {
            envText.value = await invoke<string>('generate_env', {
                instanceId: props.instanceId,
                credentialId: issued.credential.id,
                format: 'laravel',
            })
        }
    } catch (cause) {
        setError(cause)
    } finally {
        working.value = false
    }
}
</script>

<template>
    <UModal v-model:open="open" :title="t('wizard.title')" :ui="{content: 'max-w-3xl'}">
        <template #body>
            <div class="space-y-4">
                <div class="flex items-center gap-2">
                    <UBadge
                        v-for="(item, index) in steps"
                        :key="item"
                        :color="index === step ? 'primary' : index < step ? 'success' : 'neutral'"
                        variant="subtle"
                        size="sm"
                    >
                        {{ index + 1 }}. {{ t(`wizard.steps.${item}`) }}
                    </UBadge>
                </div>

                <div v-if="current === 'bucket'" class="space-y-3">
                    <UFormField :label="t('wizard.template')">
                        <USelect
                            v-model="template"
                            :items="[
                                {label: 'Laravel media', value: 'laravel-media'},
                                {label: t('wizard.templateBlank'), value: 'blank'},
                            ]"
                            class="w-full"
                        />
                    </UFormField>
                    <UFormField :label="t('wizard.bucket')">
                        <USelect
                            v-model="mode"
                            :items="[
                                {label: t('wizard.bucketNew'), value: 'new'},
                                {label: t('wizard.bucketExisting'), value: 'existing'},
                            ]"
                            class="w-full"
                        />
                    </UFormField>
                    <UFormField v-if="mode === 'new'" :label="t('wizard.bucketName')">
                        <UInput v-model="name" :placeholder="t('wizard.bucketPlaceholder')"/>
                    </UFormField>
                    <UFormField v-else :label="t('wizard.bucketExisting')">
                        <USelect
                            v-model="name"
                            :items="(instanceId ? buckets(instanceId) : []).map((item) => ({label: item, value: item}))"
                            class="w-full"
                        />
                    </UFormField>
                </div>

                <div v-else-if="current === 'visibility'" class="space-y-3">
                    <p class="text-sm text-muted">{{ t('wizard.visibilityHint') }}</p>
                    <div class="flex items-end gap-2">
                        <UFormField :label="t('wizard.prefix')" class="flex-1">
                            <UInput v-model="newPrefix" :placeholder="t('wizard.prefixPlaceholder')" @keyup.enter="addPrefix"/>
                        </UFormField>
                        <UButton :label="t('wizard.add')" color="neutral" variant="subtle" @click="addPrefix"/>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <UBadge v-for="prefix in prefixes" :key="prefix" color="neutral" variant="subtle" class="gap-1">
                            <span class="font-mono text-xs">{{ prefix }}</span>
                            <UButton
                                icon="i-lucide-x"
                                color="neutral"
                                variant="link"
                                size="xs"
                                @click="prefixes = prefixes.filter((item) => item !== prefix)"
                            />
                        </UBadge>
                    </div>
                    <p class="text-xs text-muted">{{ t('wizard.privateHint') }}</p>
                </div>

                <div v-else-if="current === 'credentials'" class="space-y-3">
                    <UFormField :label="t('wizard.mode')">
                        <USelect
                            v-model="credentialMode"
                            :items="[
                                {label: t('access.modeRw'), value: 'rw'},
                                {label: t('access.modeRo'), value: 'ro'},
                                {label: t('access.modeRwNoDelete'), value: 'rw-sans-suppression'},
                            ]"
                            class="w-full"
                        />
                    </UFormField>
                    <UFormField :label="t('wizard.accessKey')">
                        <UInput v-model="accessKey" :placeholder="`${name.trim() || '<bucket>'}-app`"/>
                    </UFormField>
                </div>

                <div v-else-if="current === 'retention'" class="space-y-3">
                    <p class="text-sm text-muted">{{ t('wizard.retentionHint') }}</p>
                    <div class="flex items-end gap-2">
                        <UFormField :label="t('wizard.prefix')" class="flex-1">
                            <UInput v-model="rulePrefix" :placeholder="t('wizard.prefixPlaceholder')"/>
                        </UFormField>
                        <UFormField :label="t('wizard.days')">
                            <UInput v-model="ruleDays" type="number" min="1" class="w-24"/>
                        </UFormField>
                        <UButton :label="t('wizard.add')" color="neutral" variant="subtle" @click="addRule"/>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <UBadge v-for="rule in rules" :key="rule.prefix" color="neutral" variant="subtle" class="gap-1">
                            <span>{{ rule.prefix || t('lifecycle.bucketRoot') }} · {{ rule.expireDays }} {{ t('lifecycle.daysShort') }}</span>
                            <UButton
                                icon="i-lucide-x"
                                color="neutral"
                                variant="link"
                                size="xs"
                                @click="rules = rules.filter((item) => item.prefix !== rule.prefix)"
                            />
                        </UBadge>
                    </div>
                    <p class="text-xs text-muted">{{ t('lifecycle.multipartNote') }}</p>
                </div>

                <div v-else class="space-y-3">
                    <p class="text-sm">{{ t('wizard.summary', {bucket: name.trim()}) }}</p>
                    <ul class="list-inside list-disc text-sm text-muted">
                        <li>{{ t('wizard.summaryBucket', {bucket: name.trim()}) }}</li>
                        <li>{{ t('wizard.summaryPolicy', {n: prefixes.length}) }}</li>
                        <li>{{ t('wizard.summaryUser', {name: accessKey.trim() || `${name.trim()}-app`}) }}</li>
                        <li>{{ t('wizard.summaryLifecycle', {n: rules.length}) }}</li>
                    </ul>
                    <div v-if="previewPolicy?.changed || previewConfig?.rulesChanged" class="text-sm">
                        <p class="font-medium">{{ t('wizard.changes') }}</p>
                        <p class="text-muted">{{ t('wizard.policyDiff', {n: previewPolicy?.diff.length ?? 0}) }}</p>
                        <p class="text-muted">{{ t('wizard.lifecycleDiff', {n: previewConfig?.ruleDiff.length ?? 0}) }}</p>
                    </div>
                    <details>
                        <summary class="cursor-pointer text-sm">{{ t('wizard.rcEquivalent') }}</summary>
                        <pre class="mt-2 overflow-x-auto rounded bg-elevated p-2 text-xs">{{ rcCommands }}</pre>
                    </details>

                    <UAlert
                        v-if="results.length"
                        color="success"
                        variant="subtle"
                        icon="i-lucide-circle-check"
                        :title="t('wizard.applied')"
                    >
                        <template #description>
                            <ul class="list-inside list-disc">
                                <li v-for="result in results" :key="result">{{ result }}</li>
                            </ul>
                        </template>
                    </UAlert>

                    <div v-if="envText">
                        <p class="mb-1 text-sm font-medium">{{ t('wizard.envReady') }}</p>
                        <pre class="overflow-x-auto rounded bg-elevated p-2 text-xs">{{ envText }}</pre>
                        <p class="mt-1 text-xs text-muted">{{ t('env.secretWarning') }}</p>
                    </div>
                </div>

                <UAlert
                    v-if="error"
                    color="error"
                    variant="subtle"
                    icon="i-lucide-circle-alert"
                    :title="error.message"
                    :description="error.hint"
                />
            </div>
        </template>
        <template #footer>
            <div class="flex w-full justify-between gap-2">
                <UButton
                    :label="t('wizard.back')"
                    color="neutral"
                    variant="ghost"
                    :disabled="step === 0 || working"
                    @click="back"
                />
                <div class="flex gap-2">
                    <UButton
                        :label="t('wizard.cancel')"
                        color="neutral"
                        variant="ghost"
                        @click="open = false"
                    />
                    <UButton
                        v-if="step < steps.length - 1"
                        :label="t('wizard.next')"
                        :disabled="!canContinue() || working"
                        :loading="working"
                        @click="next"
                    />
                    <UButton
                        v-else
                        icon="i-lucide-rocket"
                        :label="t('wizard.apply')"
                        :loading="working"
                        :disabled="results.length > 0"
                        @click="run"
                    />
                </div>
            </div>
        </template>
    </UModal>
</template>
