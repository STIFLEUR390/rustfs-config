<script setup lang="ts">
import {open as openFileDialog, save as saveFileDialog} from '@tauri-apps/plugin-dialog'
import type {ConnectionStatus} from '~/composables/useInstances'
import {instanceStatus} from '~/composables/useInstances'

const {t} = useI18n()
const {activeInstance} = useInstances()
const {
    activeBucketName,
    activePrefix,
    page,
    loading,
    error,
    loadObjects,
    openPrefix,
    upload,
    download,
    deleteObject,
} = useBucket()

const tab = ref('overview')
const busy = ref(false)
const actionError = ref<{message?: string; hint?: string} | null>(null)
const bucketDialogOpen = ref(false)
const wizardOpen = ref(false)
const pendingDelete = ref<string | null>(null)
const confirmation = ref('')
const deleteOpen = computed({
    get: () => pendingDelete.value !== null,
    set: (value: boolean) => {
        if (!value) pendingDelete.value = null
    },
})

const tabs = computed(() => [
    {label: t('tabs.overview'), value: 'overview', icon: 'i-lucide-layout-dashboard'},
    {label: t('tabs.objects'), value: 'objects', icon: 'i-lucide-folder'},
    {label: t('tabs.access'), value: 'access', icon: 'i-lucide-key-round'},
    {label: t('tabs.policy'), value: 'policy', icon: 'i-lucide-shield'},
    {label: t('tabs.lifecycle'), value: 'lifecycle', icon: 'i-lucide-clock'},
    {label: t('tabs.env'), value: 'env', icon: 'i-lucide-file-code'},
])

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

const crumbs = computed(() =>
    activePrefix.value
        .split('/')
        .filter(Boolean)
        .map((part, index, parts) => ({
            label: part,
            prefix: `${parts.slice(0, index + 1).join('/')}/`,
        })),
)

const listError = computed(() => (error.value as {message?: string; hint?: string} | null))

watch([tab, activeBucketName], ([value]) => {
    if (value === 'objects' && activeInstance.value) {
        loadObjects(activeInstance.value.id)
    }
})

function relative(key: string) {
    return key.slice(activePrefix.value.length)
}

function formatSize(size: number) {
    if (size < 1024) return `${size} B`
    const units = ['KB', 'MB', 'GB', 'TB']
    let value = size / 1024
    let unit = 0
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024
        unit += 1
    }
    return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`
}

function formatDate(value: string | null) {
    return value ? new Date(value).toLocaleString() : '—'
}

async function uploadFile() {
    if (!activeInstance.value) return
    const path = await openFileDialog({multiple: false})
    if (typeof path !== 'string') return
    busy.value = true
    actionError.value = null
    try {
        await upload(activeInstance.value.id, path)
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}

async function downloadObject(key: string) {
    if (!activeInstance.value) return
    const path = await saveFileDialog({defaultPath: relative(key)})
    if (!path) return
    busy.value = true
    actionError.value = null
    try {
        await download(activeInstance.value.id, key, path)
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}

function askDelete(key: string) {
    pendingDelete.value = key
    confirmation.value = ''
}

async function confirmDelete() {
    if (!activeInstance.value || !pendingDelete.value) return
    busy.value = true
    actionError.value = null
    try {
        await deleteObject(activeInstance.value.id, pendingDelete.value)
        pendingDelete.value = null
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}

function loadMore() {
    if (activeInstance.value && page.value.nextToken) {
        loadObjects(activeInstance.value.id, page.value.nextToken)
    }
}

function refresh() {
    if (activeInstance.value) loadObjects(activeInstance.value.id)
}
</script>

<template>
    <div class="flex h-full flex-col">
        <header class="flex h-14 shrink-0 items-center gap-2 border-b border-default px-5">
            <template v-if="activeInstance">
                <span class="font-medium text-highlighted">{{ activeInstance.label }}</span>
                <span class="text-dimmed">·</span>
                <span class="text-muted">{{ activeBucketName ?? t('sidebar.noBuckets') }}</span>
                <UButton
                    class="ml-auto"
                    icon="i-lucide-wand-2"
                    size="xs"
                    :label="t('wizard.open')"
                    @click="wizardOpen = true"
                />
                <UButton
                    v-if="activeBucketName"
                    icon="i-lucide-trash-2"
                    color="error"
                    variant="ghost"
                    size="xs"
                    :title="t('buckets.deleteTitle')"
                    @click="bucketDialogOpen = true"
                />
            </template>
            <span v-else class="text-muted">{{ t('empty.noBucket') }}</span>
        </header>

        <div class="flex-1 overflow-y-auto p-5">
            <div v-if="!activeInstance || !activeBucketName" class="mt-24 flex flex-col items-center gap-2 text-center">
                <UIcon name="i-lucide-database" class="size-8 text-dimmed"/>
                <p class="text-muted">{{ t('empty.noBucket') }}</p>
            </div>

            <template v-else>
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
                                    {{ t('overview.bucket') }} · {{ activeBucketName }}
                                </p>
                            </template>
                            <p class="text-sm text-muted">{{ t('overview.bucketPending') }}</p>
                        </UCard>

                        <UAlert
                            class="md:col-span-2"
                            :color="statusMeta.color"
                            :icon="statusMeta.icon"
                            :title="statusMeta.title"
                            :description="statusMeta.description"
                        />
                    </div>

                    <div v-else-if="tab === 'objects'" class="space-y-3">
                        <div class="flex flex-wrap items-center gap-2">
                            <UButton
                                icon="i-lucide-upload"
                                :label="t('objects.upload')"
                                :loading="busy"
                                @click="uploadFile"
                            />
                            <UButton
                                icon="i-lucide-refresh-cw"
                                color="neutral"
                                variant="outline"
                                size="sm"
                                :label="t('objects.refresh')"
                                :loading="loading"
                                @click="refresh"
                            />
                        </div>

                        <nav class="flex flex-wrap items-center gap-1 text-sm">
                            <UButton
                                size="xs"
                                color="neutral"
                                variant="ghost"
                                :label="activeBucketName"
                                @click="openPrefix('')"
                            />
                            <template v-for="crumb in crumbs" :key="crumb.prefix">
                                <UIcon name="i-lucide-chevron-right" class="size-3 text-dimmed"/>
                                <UButton
                                    size="xs"
                                    color="neutral"
                                    variant="ghost"
                                    :label="crumb.label"
                                    @click="openPrefix(crumb.prefix)"
                                />
                            </template>
                        </nav>

                        <UAlert
                            v-if="listError?.message"
                            color="error"
                            icon="i-lucide-circle-alert"
                            :title="listError.message"
                            :description="listError.hint"
                        />
                        <UAlert
                            v-if="actionError?.message"
                            color="error"
                            icon="i-lucide-circle-alert"
                            :title="actionError.message"
                            :description="actionError.hint"
                        />

                        <div class="divide-y divide-default rounded-lg border border-default">
                            <button
                                v-for="prefix in page.prefixes"
                                :key="prefix"
                                type="button"
                                class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-elevated/60"
                                @click="openPrefix(prefix)"
                            >
                                <UIcon name="i-lucide-folder" class="size-4 text-primary"/>
                                <span class="truncate">{{ relative(prefix) }}</span>
                            </button>

                            <div
                                v-for="entry in page.entries"
                                :key="entry.key"
                                class="flex items-center gap-2 px-3 py-2 text-sm"
                            >
                                <UIcon name="i-lucide-file" class="size-4 text-dimmed"/>
                                <span class="min-w-0 flex-1 truncate">{{ relative(entry.key) }}</span>
                                <span class="hidden text-xs text-muted sm:inline">{{ formatSize(entry.size) }}</span>
                                <span class="hidden text-xs text-muted md:inline">{{ formatDate(entry.lastModified) }}</span>
                                <UButton
                                    icon="i-lucide-download"
                                    color="neutral"
                                    variant="ghost"
                                    size="xs"
                                    :title="t('objects.download')"
                                    @click="downloadObject(entry.key)"
                                />
                                <UButton
                                    icon="i-lucide-trash-2"
                                    color="error"
                                    variant="ghost"
                                    size="xs"
                                    :title="t('objects.delete')"
                                    @click="askDelete(entry.key)"
                                />
                            </div>

                            <p
                                v-if="!page.prefixes.length && !page.entries.length && !loading"
                                class="px-3 py-8 text-center text-sm text-muted"
                            >
                                {{ t('objects.empty') }}
                            </p>
                        </div>

                        <UButton
                            v-if="page.nextToken"
                            block
                            color="neutral"
                            variant="outline"
                            :label="t('objects.loadMore')"
                            :loading="loading"
                            @click="loadMore"
                        />
                    </div>

                    <div v-else-if="tab === 'access'">
                        <AccessPanel :instance-id="activeInstance.id" :bucket="activeBucketName"/>
                    </div>

                    <div v-else-if="tab === 'policy'">
                        <PolicyPanel :instance-id="activeInstance.id" :bucket="activeBucketName"/>
                    </div>

                    <div v-else-if="tab === 'lifecycle'">
                        <LifecyclePanel :instance-id="activeInstance.id" :bucket="activeBucketName"/>
                    </div>

                    <div v-else-if="tab === 'env'">
                        <EnvPanel :instance-id="activeInstance.id" :bucket="activeBucketName"/>
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

        <WizardFlow
            v-if="activeInstance"
            v-model:open="wizardOpen"
            :instance-id="activeInstance.id"
            @done="refresh"
        />

        <BucketDialog
            v-if="activeInstance && activeBucketName"
            v-model:open="bucketDialogOpen"
            mode="delete"
            :instance-id="activeInstance.id"
            :bucket="activeBucketName"
            @done="refresh"
        />

        <UModal v-model:open="deleteOpen" :title="t('objects.deleteTitle')">
            <template #body>
                <div class="space-y-3">
                    <p class="text-sm">{{ t('objects.deleteHint', {key: pendingDelete}) }}</p>
                    <UFormField :label="t('objects.confirmLabel')" name="key">
                        <UInput v-model="confirmation" class="w-full font-mono" autocomplete="off"/>
                    </UFormField>
                </div>
            </template>
            <template #footer>
                <div class="ml-auto flex gap-2">
                    <UButton
                        color="neutral"
                        variant="ghost"
                        :label="t('instanceForm.cancel')"
                        @click="pendingDelete = null"
                    />
                    <UButton
                        color="error"
                        :label="t('objects.delete')"
                        :loading="busy"
                        :disabled="confirmation !== pendingDelete"
                        @click="confirmDelete"
                    />
                </div>
            </template>
        </UModal>
    </div>
</template>
