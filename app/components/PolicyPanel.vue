<script setup lang="ts">
import type {PolicyPlan} from '~/composables/useBucket'

const props = defineProps<{instanceId: string; bucket: string}>()
const {t} = useI18n()
const {bucketState, planPolicy, applyPolicy} = useBucket()

const prefixes = ref<string[]>([])
const newPrefix = ref('')
const wholeBucket = ref(false)
const plan = ref<PolicyPlan | null>(null)
const loading = ref(false)
const applying = ref(false)
const error = ref<{message?: string; hint?: string} | null>(null)
const applied = ref(false)
const confirmOpen = ref(false)
const confirmation = ref('')

const effectivePrefixes = computed(() => (wholeBucket.value ? [''] : prefixes.value))

async function load() {
    error.value = null
    plan.value = null
    applied.value = false
    wholeBucket.value = false
    try {
        const state = await bucketState(props.instanceId, props.bucket)
        prefixes.value = state?.publicPrefixes ?? []
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    }
}

watch(() => [props.instanceId, props.bucket], load, {immediate: true})

// A stale plan would let "apply" skip the whole-bucket confirmation.
watch([wholeBucket, prefixes], () => {
    plan.value = null
    applied.value = false
})

function addPrefix() {
    const value = newPrefix.value.trim()
    if (!value) return
    const normalized = value.endsWith('/') ? value : `${value}/`
    if (!prefixes.value.includes(normalized)) prefixes.value = [...prefixes.value, normalized]
    newPrefix.value = ''
}

function removePrefix(prefix: string) {
    prefixes.value = prefixes.value.filter((item) => item !== prefix)
}

async function preview() {
    loading.value = true
    error.value = null
    applied.value = false
    try {
        plan.value = await planPolicy(props.instanceId, props.bucket, effectivePrefixes.value)
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    } finally {
        loading.value = false
    }
}

async function apply() {
    if (wholeBucket.value || plan.value?.requiresConfirmation) {
        confirmation.value = ''
        confirmOpen.value = true
        return
    }
    await runApply(false)
}

async function runApply(allowRoot: boolean) {
    applying.value = true
    error.value = null
    try {
        const result = await applyPolicy(
            props.instanceId,
            props.bucket,
            effectivePrefixes.value,
            allowRoot,
        )
        plan.value = result
        applied.value = true
        confirmOpen.value = false
    } catch (cause) {
        error.value = cause as {message?: string; hint?: string}
    } finally {
        applying.value = false
    }
}

function actionColor(action: string) {
    if (action === 'add') return 'success' as const
    if (action === 'remove') return 'error' as const
    return 'warning' as const
}

function actionLabel(action: string) {
    if (action === 'add') return t('policy.addAction')
    if (action === 'remove') return t('policy.removeAction')
    return t('policy.updateAction')
}
</script>

<template>
    <div class="space-y-4">
        <UCard>
            <template #header>
                <div>
                    <h3 class="font-medium">{{ t('policy.title') }}</h3>
                    <p class="text-sm text-muted">{{ t('policy.hint') }}</p>
                </div>
            </template>

            <div class="space-y-4">
                <div class="flex items-end gap-2">
                    <UFormField :label="t('policy.prefix')" class="flex-1">
                        <UInput
                            v-model="newPrefix"
                            :placeholder="t('policy.prefixPlaceholder')"
                            :disabled="wholeBucket"
                            @keyup.enter="addPrefix"
                        />
                    </UFormField>
                    <UButton
                        icon="i-lucide-plus"
                        :label="t('policy.add')"
                        color="neutral"
                        variant="subtle"
                        :disabled="wholeBucket || !newPrefix.trim()"
                        @click="addPrefix"
                    />
                </div>

                <div v-if="prefixes.length" class="flex flex-wrap gap-2">
                    <UBadge
                        v-for="prefix in prefixes"
                        :key="prefix"
                        color="neutral"
                        variant="subtle"
                        class="gap-1"
                    >
                        <span class="font-mono text-xs">{{ prefix }}</span>
                        <UButton
                            icon="i-lucide-x"
                            color="neutral"
                            variant="link"
                            size="xs"
                            :aria-label="t('policy.remove')"
                            @click="removePrefix(prefix)"
                        />
                    </UBadge>
                </div>

                <div class="space-y-2">
                    <UCheckbox v-model="wholeBucket" :label="t('policy.wholeBucket')"/>
                    <UAlert
                        v-if="wholeBucket"
                        color="error"
                        variant="subtle"
                        icon="i-lucide-triangle-alert"
                        :title="t('policy.wholeBucketWarning')"
                    />
                </div>

                <UAlert
                    v-if="error"
                    color="error"
                    variant="subtle"
                    icon="i-lucide-circle-alert"
                    :title="error.message ?? t('policy.error')"
                    :description="error.hint"
                />

                <UAlert
                    v-if="applied"
                    color="success"
                    variant="subtle"
                    icon="i-lucide-circle-check"
                    :title="t('policy.applied')"
                />

                <div class="flex gap-2">
                    <UButton
                        icon="i-lucide-git-compare"
                        :label="t('policy.preview')"
                        :loading="loading"
                        @click="preview"
                    />
                    <UButton
                        v-if="plan"
                        icon="i-lucide-check"
                        color="primary"
                        :label="t('policy.apply')"
                        :loading="applying"
                        :disabled="!plan.changed"
                        @click="apply"
                    />
                </div>
            </div>
        </UCard>

        <UCard v-if="plan">
            <template #header>
                <div class="flex items-center justify-between">
                    <h3 class="font-medium">{{ t('policy.preview') }}</h3>
                    <UBadge :color="plan.changed ? 'warning' : 'neutral'" variant="subtle">
                        {{ plan.changed ? t('policy.changes', {n: plan.diff.length}) : t('policy.noChange') }}
                    </UBadge>
                </div>
            </template>
            <div v-if="plan.diff.length" class="divide-y divide-default">
                <details v-for="change in plan.diff" :key="change.sid" class="py-2">
                    <summary class="flex cursor-pointer items-center gap-2 text-sm">
                        <UBadge :color="actionColor(change.action)" variant="subtle" size="sm">
                            {{ actionLabel(change.action) }}
                        </UBadge>
                        <span class="font-mono text-xs">{{ change.sid }}</span>
                    </summary>
                    <pre class="mt-2 overflow-x-auto rounded bg-elevated p-2 text-xs">{{
                        JSON.stringify(change.after ?? change.before, null, 2)
                    }}</pre>
                </details>
            </div>
            <p v-else class="text-sm text-muted">{{ t('policy.noChange') }}</p>
        </UCard>

        <UModal v-model:open="confirmOpen" :title="t('policy.confirmTitle')">
            <template #body>
                <div class="space-y-3">
                    <p class="text-sm">{{ t('policy.confirmHint', {bucket}) }}</p>
                    <UInput v-model="confirmation" :placeholder="bucket"/>
                </div>
            </template>
            <template #footer>
                <div class="flex w-full justify-end gap-2">
                    <UButton
                        :label="t('policy.cancel')"
                        color="neutral"
                        variant="ghost"
                        @click="confirmOpen = false"
                    />
                    <UButton
                        :label="t('policy.apply')"
                        color="error"
                        :loading="applying"
                        :disabled="confirmation !== bucket"
                        @click="runApply(true)"
                    />
                </div>
            </template>
        </UModal>
    </div>
</template>
