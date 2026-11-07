<script setup lang="ts">
import {writeText} from '@tauri-apps/plugin-clipboard-manager'
import type {Credential, CredentialMode, IsolationReport} from '~/composables/useCredentials'

const props = defineProps<{instanceId: string; bucket: string}>()
const {t} = useI18n()
const {credentials, loading, error, load, create, adopt, rotate, revoke, verify} = useCredentials()

const busy = ref(false)
const actionError = ref<{message?: string; hint?: string} | null>(null)
const notice = ref<string | null>(null)

const createOpen = ref(false)
const mode = ref<CredentialMode>('rw')
const accessKey = ref('')

const adoptOpen = ref(false)
const adoptKey = ref('')

const secretOpen = ref(false)
const issued = ref<{accessKey: string; secret: string} | null>(null)

const revokeTarget = ref<Credential | null>(null)
const confirmation = ref('')

const reportOpen = ref(false)
const report = ref<IsolationReport | null>(null)
const verifyingId = ref<string | null>(null)

const modeItems = computed(() => [
    {label: t('access.modeRw'), value: 'rw'},
    {label: t('access.modeRo'), value: 'ro'},
    {label: t('access.modeRwNoDelete'), value: 'rw-sans-suppression'},
])

watch(() => [props.instanceId, props.bucket], () => load(props.instanceId, props.bucket), {immediate: true})

function resetCreate() {
    mode.value = 'rw'
    accessKey.value = ''
    actionError.value = null
}

async function submitCreate() {
    busy.value = true
    actionError.value = null
    try {
        const result = await create(props.instanceId, props.bucket, mode.value, accessKey.value)
        createOpen.value = false
        resetCreate()
        if (result.secret) {
            issued.value = {accessKey: result.credential.accessKey, secret: result.secret}
            secretOpen.value = true
        }
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}

async function submitAdopt() {
    busy.value = true
    actionError.value = null
    try {
        const result = await adopt(props.instanceId, props.bucket, adoptKey.value)
        adoptOpen.value = false
        adoptKey.value = ''
        notice.value = t('access.adopted', {key: result.credential.accessKey})
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}

async function submitRotate(credential: Credential) {
    busy.value = true
    actionError.value = null
    notice.value = null
    try {
        const secret = await rotate(props.instanceId, credential)
        issued.value = {accessKey: credential.accessKey, secret}
        secretOpen.value = true
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}

function askRevoke(credential: Credential) {
    revokeTarget.value = credential
    confirmation.value = ''
    actionError.value = null
}

async function confirmRevoke() {
    if (!revokeTarget.value) return
    busy.value = true
    actionError.value = null
    try {
        await revoke(props.instanceId, revokeTarget.value)
        revokeTarget.value = null
        notice.value = null
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        busy.value = false
    }
}

async function copySecret() {
    if (issued.value) await writeText(issued.value.secret)
}

async function runVerify(credential: Credential) {
    verifyingId.value = credential.id
    actionError.value = null
    try {
        report.value = await verify(props.instanceId, props.bucket, credential)
        reportOpen.value = true
    } catch (cause) {
        actionError.value = cause as {message?: string; hint?: string}
    } finally {
        verifyingId.value = null
    }
}

function checkIcon(status: string) {
    if (status === 'pass') return 'i-lucide-circle-check'
    if (status === 'fail') return 'i-lucide-circle-x'
    return 'i-lucide-circle-minus'
}

function checkColor(status: string) {
    if (status === 'pass') return 'success' as const
    if (status === 'fail') return 'error' as const
    return 'neutral' as const
}

function checkIconClass(status: string) {
    if (status === 'pass') return 'mt-0.5 size-4 text-success'
    if (status === 'fail') return 'mt-0.5 size-4 text-error'
    return 'mt-0.5 size-4 text-dimmed'
}

function formatDate(value: string | null) {
    return value ? new Date(value).toLocaleString() : '—'
}
</script>

<template>
    <div class="space-y-3">
        <div class="flex flex-wrap items-center gap-2">
            <div class="mr-auto">
                <p class="font-medium">{{ t('access.iamTitle') }}</p>
                <p class="text-sm text-muted">{{ t('access.iamHint') }}</p>
            </div>
            <UButton
                icon="i-lucide-user-plus"
                color="neutral"
                variant="outline"
                :label="t('access.adopt')"
                @click="adoptOpen = true"
            />
            <UButton icon="i-lucide-key-round" :label="t('access.create')" @click="createOpen = true"/>
        </div>

        <UAlert
            v-if="error?.message || actionError?.message"
            color="error"
            icon="i-lucide-circle-alert"
            :title="error?.message || actionError?.message"
            :description="error?.hint || actionError?.hint"
        />
        <UAlert v-if="notice" color="info" icon="i-lucide-info" :title="notice"/>

        <div class="divide-y divide-default rounded-lg border border-default">
            <div
                v-for="credential in credentials"
                :key="credential.id"
                class="flex flex-wrap items-center gap-2 px-3 py-2 text-sm"
            >
                <UIcon name="i-lucide-key-round" class="size-4 text-primary"/>
                <span class="font-mono">{{ credential.accessKey }}</span>
                <UBadge color="neutral" variant="subtle" size="sm">{{ credential.mode }}</UBadge>
                <span class="hidden text-xs text-muted md:inline">{{ credential.policyName }}</span>
                <span class="ml-auto hidden text-xs text-muted sm:inline">
                    {{ t('access.rotatedAt', {date: formatDate(credential.rotatedAt)}) }}
                </span>
                <UButton
                    icon="i-lucide-shield-check"
                    color="neutral"
                    variant="ghost"
                    size="xs"
                    :title="t('access.verify')"
                    :loading="verifyingId === credential.id"
                    @click="runVerify(credential)"
                />
                <UButton
                    icon="i-lucide-refresh-cw"
                    color="neutral"
                    variant="ghost"
                    size="xs"
                    :title="t('access.rotate')"
                    :loading="busy"
                    @click="submitRotate(credential)"
                />
                <UButton
                    icon="i-lucide-user-x"
                    color="error"
                    variant="ghost"
                    size="xs"
                    :title="t('access.revoke')"
                    @click="askRevoke(credential)"
                />
            </div>
            <p v-if="!credentials.length && !loading" class="px-3 py-8 text-center text-sm text-muted">
                {{ t('access.empty') }}
            </p>
            <p v-if="loading" class="px-3 py-8 text-center text-sm text-muted">{{ t('access.loading') }}</p>
        </div>

        <UModal v-model:open="createOpen" :title="t('access.createTitle')">
            <template #body>
                <div class="space-y-3">
                    <p class="text-sm text-muted">{{ t('access.createHint') }}</p>
                    <UFormField :label="t('access.mode')" name="mode">
                        <USelect v-model="mode" :items="modeItems" class="w-full"/>
                    </UFormField>
                    <UFormField :label="t('access.accessKey')" name="accessKey">
                        <UInput
                            v-model="accessKey"
                            class="w-full font-mono"
                            :placeholder="`${bucket}-app`"
                            autocomplete="off"
                        />
                    </UFormField>
                </div>
            </template>
            <template #footer>
                <div class="ml-auto flex gap-2">
                    <UButton
                        color="neutral"
                        variant="ghost"
                        :label="t('instanceForm.cancel')"
                        @click="createOpen = false"
                    />
                    <UButton :label="t('access.create')" :loading="busy" @click="submitCreate"/>
                </div>
            </template>
        </UModal>

        <UModal v-model:open="adoptOpen" :title="t('access.adoptTitle')">
            <template #body>
                <div class="space-y-3">
                    <p class="text-sm text-muted">{{ t('access.adoptHint') }}</p>
                    <UFormField :label="t('access.accessKey')" name="adoptKey">
                        <UInput v-model="adoptKey" class="w-full font-mono" autocomplete="off"/>
                    </UFormField>
                </div>
            </template>
            <template #footer>
                <div class="ml-auto flex gap-2">
                    <UButton
                        color="neutral"
                        variant="ghost"
                        :label="t('instanceForm.cancel')"
                        @click="adoptOpen = false"
                    />
                    <UButton
                        :label="t('access.adopt')"
                        :loading="busy"
                        :disabled="!adoptKey.trim()"
                        @click="submitAdopt"
                    />
                </div>
            </template>
        </UModal>

        <UModal v-model:open="secretOpen" :title="t('access.secretTitle')">            <template #body>
                <div v-if="issued" class="space-y-3">
                    <UAlert
                        color="warning"
                        icon="i-lucide-triangle-alert"
                        :title="t('access.secretWarning')"
                    />
                    <UFormField :label="t('access.accessKey')">
                        <UInput :model-value="issued.accessKey" readonly class="w-full font-mono"/>
                    </UFormField>
                    <UFormField :label="t('access.secret')">
                        <div class="flex gap-2">
                            <UInput :model-value="issued.secret" readonly class="w-full font-mono"/>
                            <UButton icon="i-lucide-copy" color="neutral" variant="outline" @click="copySecret"/>
                        </div>
                    </UFormField>
                </div>
            </template>
            <template #footer>
                <div class="ml-auto">
                    <UButton :label="t('access.done')" @click="secretOpen = false"/>
                </div>
            </template>
        </UModal>

        <UModal v-model:open="reportOpen" :title="t('access.verifyTitle')">
            <template #body>
                <div v-if="report" class="space-y-3">
                    <UAlert
                        :color="report.ok ? 'success' : 'error'"
                        :icon="report.ok ? 'i-lucide-shield-check' : 'i-lucide-shield-x'"
                        :title="report.ok ? t('access.verifyOk') : t('access.verifyFailed')"
                    />
                    <div class="divide-y divide-default rounded-lg border border-default">
                        <div
                            v-for="check in report.checks"
                            :key="check.id"
                            class="flex items-start gap-2 px-3 py-2 text-sm"
                        >
                            <UIcon :name="checkIcon(check.status)" :class="checkIconClass(check.status)"/>
                            <div class="min-w-0 flex-1">
                                <p>{{ t(`access.checks.${check.id}`) }}</p>
                                <p v-if="check.detail" class="text-xs text-muted">{{ check.detail }}</p>
                            </div>
                            <UBadge :color="checkColor(check.status)" variant="subtle" size="sm">
                                {{ t(`access.checkStatus.${check.status}`) }}
                            </UBadge>
                        </div>
                    </div>
                </div>
            </template>
            <template #footer>
                <div class="ml-auto">
                    <UButton :label="t('access.done')" @click="reportOpen = false"/>
                </div>
            </template>
        </UModal>

        <UModal
            :open="revokeTarget !== null"
            :title="t('access.revokeTitle')"
            @update:open="(value: boolean) => { if (!value) revokeTarget = null }"
        >
            <template #body>
                <div class="space-y-3">
                    <p class="text-sm">{{ t('access.revokeHint', {key: revokeTarget?.accessKey}) }}</p>
                    <UFormField :label="t('access.confirmLabel')" name="confirmation">
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
                        @click="revokeTarget = null"
                    />
                    <UButton
                        color="error"
                        :label="t('access.revoke')"
                        :loading="busy"
                        :disabled="confirmation !== revokeTarget?.accessKey"
                        @click="confirmRevoke"
                    />
                </div>
            </template>
        </UModal>
    </div>
</template>
