<script setup lang="ts">
import type {BucketSummary} from '~/composables/useBucket'
import type {ConnectionStatus} from '~/composables/useInstances'
import {instanceStatus} from '~/composables/useInstances'

const {t} = useI18n()
const {activeInstance} = useInstances()
const {activeBucketName, buckets} = useBucket()

const tab = ref('overview')

const status = computed<ConnectionStatus>(() =>
    activeInstance.value ? instanceStatus(activeInstance.value) : 'unknown',
)

const statusMeta = computed(() => {
    const check = activeInstance.value?.lastCheck
    const description = check
        ? t('overview.lastCheck', {
            date: new Date(check.at).toLocaleString(),
            n: check.buckets,
        })
        : undefined
    switch (status.value) {
        case 'ok':
            return {color: 'success' as const, icon: 'i-lucide-circle-check', title: t('overview.connOk'), description}
        case 'error':
            return {color: 'error' as const, icon: 'i-lucide-circle-alert', title: t('overview.connError'), description}
        default:
            return {color: 'neutral' as const, icon: 'i-lucide-circle-help', title: t('status.unknown'), description: undefined}
    }
})

const tabs = computed(() => [
    {label: t('tabs.overview'), value: 'overview', icon: 'i-lucide-layout-dashboard'},
    {label: t('tabs.objects'), value: 'objects', icon: 'i-lucide-folder'},
    {label: t('tabs.access'), value: 'access', icon: 'i-lucide-key-round'},
    {label: t('tabs.policy'), value: 'policy', icon: 'i-lucide-shield'},
    {label: t('tabs.lifecycle'), value: 'lifecycle', icon: 'i-lucide-clock'},
    {label: t('tabs.env'), value: 'env', icon: 'i-lucide-file-code'},
])

const bucket = computed<BucketSummary | null>(() => {
    if (!activeInstance.value) return null
    return buckets(activeInstance.value.id).find(item => item.name === activeBucketName.value) ?? null
})
</script>

<template>
    <div class="flex h-full flex-col">
        <header class="flex h-14 shrink-0 items-center gap-2 border-b border-default px-5">
            <template v-if="activeInstance">
                <span class="font-medium text-highlighted">{{ activeInstance.label }}</span>
                <span class="text-dimmed">·</span>
                <span class="text-muted">{{ activeBucketName ?? t('sidebar.noBuckets') }}</span>
            </template>
            <span v-else class="text-muted">{{ t('empty.noBucket') }}</span>
        </header>

        <div class="flex-1 overflow-y-auto p-5">
            <div v-if="!activeInstance || !bucket" class="mt-24 flex flex-col items-center gap-2 text-center">
                <UIcon name="i-lucide-database" class="size-8 text-dimmed"/>
                <p class="text-muted">{{ t('empty.noBucket') }}</p>
            </div>

            <template v-else>
                <div class="mb-4 flex flex-wrap gap-2">
                    <UButton
                        icon="i-lucide-wand-sparkles"
                        :label="t('actions.prepareBucket')"
                        :title="t('actions.soon')"
                        disabled
                    />
                    <UButton
                        icon="i-lucide-shield-check"
                        color="neutral"
                        variant="outline"
                        :label="t('actions.testAccess')"
                        :title="t('actions.soon')"
                        disabled
                    />
                </div>

                <UTabs v-model="tab" :items="tabs" :content="false"/>

                <div class="mt-4">
                    <div v-if="tab === 'overview'" class="grid gap-4 md:grid-cols-2">
                        <UCard>
                            <template #header>
                                <p class="font-medium">{{ t('overview.instance') }}</p>
                            </template>
                            <dl class="space-y-2 text-sm">
                                <div class="flex justify-between gap-4">
                                    <dt class="text-muted">{{ t('overview.endpoint') }}</dt>
                                    <dd class="truncate font-mono">{{ activeInstance.endpoint }}</dd>
                                </div>
                                <div class="flex justify-between gap-4">
                                    <dt class="text-muted">{{ t('overview.region') }}</dt>
                                    <dd>{{ activeInstance.region }}</dd>
                                </div>
                                <div class="flex justify-between gap-4">
                                    <dt class="text-muted">{{ t('overview.pathStyle') }}</dt>
                                    <dd>{{ activeInstance.pathStyle ? t('overview.yes') : t('overview.no') }}</dd>
                                </div>
                            </dl>
                        </UCard>

                        <UCard>
                            <template #header>
                                <p class="font-medium">
                                    {{ t('overview.bucket') }} · {{ bucket.name }}
                                </p>
                            </template>
                            <dl class="space-y-2 text-sm">
                                <div class="flex justify-between gap-4">
                                    <dt class="text-muted">{{ t('overview.publicPrefixes') }}</dt>
                                    <dd class="truncate font-mono">
                                        {{ bucket.publicPrefixes.join(', ') || '—' }}
                                    </dd>
                                </div>
                                <div class="flex justify-between gap-4">
                                    <dt class="text-muted">{{ t('overview.privatePrefixes') }}</dt>
                                    <dd class="truncate font-mono">
                                        {{ bucket.privatePrefixes.join(', ') || '—' }}
                                    </dd>
                                </div>
                                <div class="flex justify-between gap-4">
                                    <dt class="text-muted">{{ t('overview.lifecycle') }}</dt>
                                    <dd>
                                        {{ bucket.lifecycleDays === null
                                            ? t('overview.noLifecycle')
                                            : t('overview.days', {n: bucket.lifecycleDays}) }}
                                    </dd>
                                </div>
                            </dl>
                        </UCard>

                        <UAlert
                            class="md:col-span-2"
                            :color="statusMeta.color"
                            :icon="statusMeta.icon"
                            :title="statusMeta.title"
                            :description="statusMeta.description"
                        />
                    </div>

                    <div
                        v-else
                        class="mt-2 flex flex-col items-center gap-2 rounded-lg border border-default py-16 text-center"
                    >
                        <UIcon name="i-lucide-hammer" class="size-8 text-dimmed"/>
                        <p class="font-medium">{{ t('empty.pendingTitle') }}</p>
                        <p class="max-w-sm text-sm text-muted">{{ t('empty.pendingDescription') }}</p>
                    </div>
                </div>
            </template>
        </div>
    </div>
</template>
