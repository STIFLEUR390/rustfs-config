<script setup lang="ts">
import type {ConnectionStatus, Instance} from '~/composables/useInstances'
import {instanceStatus} from '~/composables/useInstances'

const {t, locale, setLocale} = useI18n()
const {instances, activeInstanceId, ensureLoaded, selectInstance} = useInstances()
const {activeBucketName, buckets, loadBuckets, selectBucket} = useBucket()
const formOpen = ref(false)
const editing = ref<Instance | null>(null)
const bucketDialogOpen = ref(false)
const bucketDialogMode = ref<'create' | 'delete'>('create')

const languages = [
    {code: 'fr', label: 'FR'},
    {code: 'en', label: 'EN'},
]

const statusClass: Record<ConnectionStatus, string> = {
    ok: 'bg-success',
    error: 'bg-error',
    unknown: 'bg-dimmed',
}

async function openInstance(id: string) {
    selectInstance(id)
    selectBucket(null)
    try {
        await loadBuckets(id)
    } catch {
        // The connection error is surfaced by the instance form and the panel alert.
    }
}

async function openBucket(instanceId: string, name: string) {
    selectInstance(instanceId)
    selectBucket(name)
}

function openCreate() {
    editing.value = null
    formOpen.value = true
}

function openEdit(instance: Instance) {
    editing.value = instance
    formOpen.value = true
}

function openBucketDialog() {
    bucketDialogMode.value = 'create'
    bucketDialogOpen.value = true
}

function onSaved(instance: Instance) {
    openInstance(instance.id)
}

onMounted(async () => {
    await ensureLoaded()
    if (activeInstanceId.value) await openInstance(activeInstanceId.value)
})
</script>

<template>
    <div class="flex h-screen overflow-hidden bg-default text-default">
        <aside class="flex w-64 shrink-0 flex-col border-r border-default bg-elevated/40">
            <div class="flex h-14 items-center gap-2 border-b border-default px-4">
                <UIcon name="i-lucide-boxes" class="size-5 text-primary"/>
                <span class="font-semibold">{{ t('app.name') }}</span>
            </div>

            <nav class="flex-1 space-y-1 overflow-y-auto p-2">
                <div v-for="instance in instances" :key="instance.id">
                    <div class="flex items-center gap-1">
                        <UButton
                            block
                            color="neutral"
                            :variant="instance.id === activeInstanceId ? 'soft' : 'ghost'"
                            class="min-w-0 flex-1 justify-start"
                            @click="openInstance(instance.id)"
                        >
                            <UIcon name="i-lucide-server" class="size-4 shrink-0"/>
                            <span class="truncate">{{ instance.label }}</span>
                            <span
                                class="ml-auto size-2 shrink-0 rounded-full"
                                :class="statusClass[instanceStatus(instance)]"
                                :title="t(`status.${instanceStatus(instance)}`)"
                            />
                        </UButton>
                        <UButton
                            icon="i-lucide-pencil"
                            color="neutral"
                            variant="ghost"
                            size="xs"
                            :title="t('sidebar.editInstance')"
                            @click="openEdit(instance)"
                        />
                    </div>

                    <div v-if="instance.id === activeInstanceId" class="mt-1 ml-4 space-y-0.5">
                        <div class="flex items-center justify-between pr-1">
                            <span class="px-2 text-xs uppercase text-dimmed">
                                {{ t('buckets.title') }}
                            </span>
                            <UButton
                                icon="i-lucide-plus"
                                color="neutral"
                                variant="ghost"
                                size="xs"
                                :title="t('buckets.createTitle')"
                                @click="openBucketDialog"
                            />
                        </div>
                        <UButton
                            v-for="name in buckets(instance.id)"
                            :key="name"
                            block
                            size="sm"
                            color="neutral"
                            :variant="name === activeBucketName ? 'soft' : 'ghost'"
                            class="justify-start"
                            @click="openBucket(instance.id, name)"
                        >
                            <UIcon name="i-lucide-database" class="size-4 shrink-0"/>
                            <span class="truncate">{{ name }}</span>
                        </UButton>
                        <p v-if="!buckets(instance.id).length" class="px-2 py-1 text-xs text-muted">
                            {{ t('sidebar.noBuckets') }}
                        </p>
                    </div>
                </div>

                <p v-if="!instances.length" class="px-2 py-3 text-sm text-muted">
                    {{ t('sidebar.noInstances') }}
                </p>
            </nav>

            <div class="space-y-2 border-t border-default p-3">
                <UButton
                    block
                    color="neutral"
                    variant="outline"
                    icon="i-lucide-plus"
                    :label="t('sidebar.addInstance')"
                    @click="openCreate"
                />
                <div class="flex items-center justify-between">
                    <span class="text-xs text-muted">{{ t('app.name') }}</span>
                    <div class="flex gap-1">
                        <UButton
                            v-for="language in languages"
                            :key="language.code"
                            size="xs"
                            :color="locale === language.code ? 'primary' : 'neutral'"
                            :variant="locale === language.code ? 'solid' : 'ghost'"
                            :label="language.label"
                            @click="setLocale(language.code)"
                        />
                    </div>
                </div>
            </div>
        </aside>

        <main class="min-w-0 flex-1 overflow-y-auto">
            <slot/>
        </main>

        <InstanceForm v-model:open="formOpen" :instance="editing" @saved="onSaved"/>

        <BucketDialog
            v-if="activeInstanceId"
            v-model:open="bucketDialogOpen"
            :mode="bucketDialogMode"
            :instance-id="activeInstanceId"
        />
    </div>
</template>
