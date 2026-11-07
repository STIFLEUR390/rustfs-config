<script setup lang="ts">
import type {BucketConfigPlan, ConfigDiff, CorsSpec} from '~/composables/useBucket'

const props = defineProps<{instanceId: string; bucket: string}>()
const {t} = useI18n()
const {bucketState, planBucketConfig, applyBucketConfig} = useBucket()

const methods = ['GET', 'PUT', 'POST', 'DELETE', 'HEAD'] as const

const rules = ref<{prefix: string; expireDays: number}[]>([])
const prefix = ref('')
const days = ref('180')
const versioning = ref(false)
const corsEnabled = ref(false)
const origins = ref('')
const corsMethods = ref<string[]>(['GET'])
const headers = ref('*')
const maxAge = ref('')

const plan = ref<BucketConfigPlan | null>(null)
const loading = ref(false)
const applying = ref(false)
const error = ref<{message?: string; hint?: string} | null>(null)
const applied = ref(false)
const confirmOpen = ref(false)
const confirmation = ref('')

const corsSpec = computed<CorsSpec | null>(() =>
    corsEnabled.value
        ? {
              origins: splitList(origins.value),
              methods: corsMethods.value,
              headers: splitList(headers.value),
              maxAgeSeconds: maxAge.value.trim() ? Number(maxAge.value) : null,
          }
        : null,
)

function splitList(value: string) {
    return value
        .split(',')
        .map((item) => item.trim())
        .filter(Boolean)
}

async function load() {
    error.value = null
    plan.value = null
    applied.value = false
    try {
        const state = await bucketState(props.instanceId, props.bucket)
        rules.value = state?.lifecycle ?? []
        await preview()
        if (plan.value) {
            versioning.value = plan.value.versioning === 'Enabled'
            const remote = plan.value.corsBefore[0] as
                | {
                      AllowedOrigins?: string[]
                      AllowedMethods?: string[]
                      AllowedHeaders?: string[]
                      MaxAgeSeconds?: number | null
                  }
                | undefined
            if (remote) {
                corsEnabled.value = true
                origins.value = (remote.AllowedOrigins ?? []).join(', ')
                corsMethods.value = (remote.AllowedMethods ?? []).map((method) => method.toUpperCase())
                headers.value = (remote.AllowedHeaders ?? []).join(', ')
                maxAge.value = remote.MaxAgeSeconds ? String(remote.MaxAgeSeconds) : ''
            }
        }
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    }
}

watch(() => [props.instanceId, props.bucket], load, {immediate: true})

function addRule() {
    const value = prefix.value.trim()
    const expireDays = Number(days.value)
    if (!Number.isInteger(expireDays) || expireDays <= 0) {
        error.value = {message: t('lifecycle.daysInvalid')}
        return
    }
    if (rules.value.some((rule) => rule.prefix === value)) return
    rules.value = [...rules.value, {prefix: value, expireDays}]
    prefix.value = ''
    error.value = null
}

function removeRule(target: string) {
    rules.value = rules.value.filter((rule) => rule.prefix !== target)
}

async function preview() {
    loading.value = true
    error.value = null
    applied.value = false
    try {
        plan.value = await planBucketConfig(
            props.instanceId,
            props.bucket,
            rules.value,
            versioning.value,
            corsSpec.value,
        )
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    } finally {
        loading.value = false
    }
}

async function apply() {
    if (!plan.value) return
    if (plan.value.corsConflict) {
        confirmation.value = ''
        confirmOpen.value = true
        return
    }
    await runApply(false)
}

async function runApply(confirmCors: boolean) {
    applying.value = true
    error.value = null
    try {
        plan.value = await applyBucketConfig(
            props.instanceId,
            props.bucket,
            rules.value,
            versioning.value,
            corsSpec.value,
            confirmCors,
        )
        applied.value = true
        confirmOpen.value = false
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    } finally {
        applying.value = false
    }
}

const hasChanges = computed(
    () =>
        !!plan.value &&
        (plan.value.rulesChanged ||
            plan.value.versioningTarget !== null ||
            plan.value.corsChanged),
)

function actionColor(action: string) {
    if (action === 'add') return 'success' as const
    if (action === 'remove') return 'error' as const
    return 'warning' as const
}

function ruleDays(diff: ConfigDiff) {
    if (diff.action === 'remove' || !diff.after) return ''
    const days = (diff.after as {Expiration?: {Days?: number}}).Expiration?.Days
    return days ? `${days} ${t('lifecycle.daysShort')}` : ''
}

function actionLabel(action: string) {
    if (action === 'add') return t('lifecycle.addAction')
    if (action === 'remove') return t('lifecycle.removeAction')
    return t('lifecycle.updateAction')
}
</script>

<template>
    <div class="space-y-4">
        <UCard>
            <template #header>
                <div>
                    <h3 class="font-medium">{{ t('lifecycle.title') }}</h3>
                    <p class="text-sm text-muted">{{ t('lifecycle.hint') }}</p>
                </div>
            </template>

            <div class="space-y-4">
                <div class="flex items-end gap-2">
                    <UFormField :label="t('lifecycle.prefix')" class="flex-1">
                        <UInput
                            v-model="prefix"
                            :placeholder="t('lifecycle.prefixPlaceholder')"
                            @keyup.enter="addRule"
                        />
                    </UFormField>
                    <UFormField :label="t('lifecycle.days')">
                        <UInput v-model="days" type="number" min="1" class="w-24"/>
                    </UFormField>
                    <UButton
                        icon="i-lucide-plus"
                        :label="t('lifecycle.add')"
                        color="neutral"
                        variant="subtle"
                        :disabled="!Number(days) || Number(days) < 1"
                        @click="addRule"
                    />
                </div>

                <div v-if="rules.length" class="flex flex-wrap gap-2">
                    <UBadge
                        v-for="rule in rules"
                        :key="rule.prefix"
                        color="neutral"
                        variant="subtle"
                        class="gap-1"
                    >
                        <span>{{ rule.prefix || t('lifecycle.bucketRoot') }} · {{ rule.expireDays }} {{ t('lifecycle.daysShort') }}</span>
                        <UButton
                            icon="i-lucide-x"
                            color="neutral"
                            variant="link"
                            size="xs"
                            :aria-label="t('lifecycle.remove')"
                            @click="removeRule(rule.prefix)"
                        />
                    </UBadge>
                </div>
                <p class="text-xs text-muted">{{ t('lifecycle.multipartNote') }}</p>

                <USeparator/>

                <div class="space-y-2">
                    <UCheckbox v-model="versioning" :label="t('lifecycle.versioning')"/>
                    <p class="text-xs text-muted">
                        {{ t('lifecycle.versioningHint') }}
                        {{ plan?.versioning ?? t('lifecycle.versioningUnset') }}
                    </p>
                </div>

                <USeparator/>

                <div class="space-y-3">
                    <UCheckbox v-model="corsEnabled" :label="t('lifecycle.cors')"/>
                    <div v-if="corsEnabled" class="grid gap-3 sm:grid-cols-2">
                        <UFormField :label="t('lifecycle.origins')">
                            <UInput
                                v-model="origins"
                                :placeholder="t('lifecycle.originsPlaceholder')"
                                class="w-full"
                            />
                        </UFormField>
                        <UFormField :label="t('lifecycle.headers')">
                            <UInput
                                v-model="headers"
                                :placeholder="t('lifecycle.headersPlaceholder')"
                                class="w-full"
                            />
                        </UFormField>
                        <UFormField :label="t('lifecycle.methods')" class="sm:col-span-2">
                            <div class="flex flex-wrap gap-3">
                                <UCheckbox
                                    v-for="method in methods"
                                    :key="method"
                                    v-model="corsMethods"
                                    :value="method"
                                    :label="method"
                                />
                            </div>
                        </UFormField>
                        <UFormField :label="t('lifecycle.maxAge')">
                            <UInput v-model="maxAge" type="number" min="0" class="w-32"/>
                        </UFormField>
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
                <UAlert
                    v-else-if="plan && !hasChanges && !applied"
                    color="success"
                    variant="subtle"
                    icon="i-lucide-check"
                    :title="t('lifecycle.noChange')"
                />
                <UAlert
                    v-else-if="applied"
                    color="success"
                    variant="subtle"
                    icon="i-lucide-check"
                    :title="t('lifecycle.applied')"
                    :description="
                        plan?.probe
                            ? plan.probe.ok
                                ? t('lifecycle.probeOk')
                                : t('lifecycle.probeMissing')
                            : undefined
                    "
                />

                <div v-if="plan && (plan.ruleDiff.length || plan.versioningTarget || plan.corsChanged)">
                    <p class="mb-2 text-sm font-medium">{{ t('lifecycle.changes') }}</p>
                    <ul class="space-y-2 text-sm">
                        <li
                            v-for="diff in plan.ruleDiff"
                            :key="diff.id"
                            class="flex flex-wrap items-center gap-2"
                        >
                            <UBadge :color="actionColor(diff.action)" variant="subtle" size="sm">
                                {{ actionLabel(diff.action) }}
                            </UBadge>
                            <code class="text-xs">{{ diff.id }}</code>
                            <span v-if="ruleDays(diff)" class="text-xs text-muted">
                                {{ ruleDays(diff) }}
                            </span>
                        </li>
                        <li v-if="plan.versioningTarget" class="flex items-center gap-2">
                            <UBadge color="warning" variant="subtle" size="sm">
                                {{ t('lifecycle.updateAction') }}
                            </UBadge>
                            <code class="text-xs">versioning → {{ plan.versioningTarget }}</code>
                        </li>
                        <li v-if="plan.corsChanged" class="flex items-center gap-2">
                            <UBadge color="warning" variant="subtle" size="sm">
                                {{ t('lifecycle.updateAction') }}
                            </UBadge>
                            <code class="text-xs">cors</code>
                            <UBadge v-if="plan.corsConflict" color="error" variant="subtle" size="sm">
                                {{ t('lifecycle.corsConflict') }}
                            </UBadge>
                        </li>
                    </ul>
                </div>

                <div class="flex flex-wrap justify-end gap-2">
                    <UButton
                        :label="t('lifecycle.preview')"
                        color="neutral"
                        variant="outline"
                        :loading="loading"
                        icon="i-lucide-search"
                        @click="preview"
                    />
                    <UButton
                        :label="t('lifecycle.apply')"
                        :disabled="!hasChanges"
                        :loading="applying"
                        icon="i-lucide-play"
                        @click="apply"
                    />
                </div>
            </div>
        </UCard>

        <UModal v-model:open="confirmOpen" :title="t('lifecycle.corsConflictTitle')">
            <template #body>
                <div class="space-y-3">
                    <p class="text-sm">{{ t('lifecycle.corsConflictHint', {bucket: props.bucket}) }}</p>
                    <UInput v-model="confirmation" :placeholder="props.bucket" class="w-full"/>
                </div>
            </template>
            <template #footer>
                <div class="flex w-full justify-end gap-2">
                    <UButton
                        :label="t('lifecycle.cancel')"
                        color="neutral"
                        variant="ghost"
                        @click="confirmOpen = false"
                    />
                    <UButton
                        :label="t('lifecycle.apply')"
                        color="error"
                        :disabled="confirmation !== props.bucket"
                        :loading="applying"
                        @click="runApply(true)"
                    />
                </div>
            </template>
        </UModal>
    </div>
</template>
