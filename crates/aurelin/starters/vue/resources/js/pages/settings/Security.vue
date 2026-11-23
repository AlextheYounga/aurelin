<script setup lang="ts">
import { Form, Head, Link, router } from '@inertiajs/vue3';
import { Passkeys } from '@laravel/passkeys';
import { ref } from 'vue';

const props = defineProps<{
    errors: Record<string, string>;
    status?: string;
    csrf_token: string;
    passwordRules: string;
    canManagePasskeys: boolean;
    passkeys: Array<{ id: number; name: string; last_used_at?: number; created_at: number }>;
    canManageTwoFactor: boolean;
    twoFactorEnabled: boolean;
    requiresConfirmation: boolean;
    twoFactorAwaitingConfirmation: boolean;
    twoFactorSecretKey?: string;
    twoFactorQrCode?: string;
    twoFactorRecoveryCodes: string[];
}>();

const passkeyName = ref('');
const registeringPasskey = ref(false);
const passkeyError = ref<string>();

const registerPasskey = async () => {
    registeringPasskey.value = true;
    passkeyError.value = undefined;
    try {
        Passkeys.configure({ fetch: { headers: { 'X-CSRF-TOKEN': props.csrf_token } } });
        await Passkeys.register({ name: passkeyName.value });
        router.reload({ only: ['passkeys'] });
        passkeyName.value = '';
    } catch (caught) {
        passkeyError.value = caught instanceof Error ? caught.message : 'The passkey could not be registered.';
    } finally {
        registeringPasskey.value = false;
    }
};
</script>

<template>
    <main>
        <Head title="Security settings" />
        <nav>
            <Link href="/dashboard">Dashboard</Link>
            <Link href="/settings/profile">Profile</Link>
            <Link href="/settings/security">Security</Link>
            <Link href="/settings/appearance">Appearance</Link>
        </nav>
        <h1>Security settings</h1>
        <p v-if="status === 'password-updated'">Password updated.</p>
        <Form action="/settings/password" method="put" :headers="{ 'x-csrf-token': csrf_token }" reset-on-success v-slot="{ processing }">
            <label for="current_password">Current password</label>
            <input id="current_password" name="current_password" type="password" required autocomplete="current-password" />
            <p v-if="errors.current_password" role="alert">{{ errors.current_password }}</p>

            <label for="password">New password</label>
            <input id="password" name="password" type="password" required autocomplete="new-password" :passwordrules="passwordRules" />
            <p v-if="errors.password" role="alert">{{ errors.password }}</p>

            <label for="password_confirmation">Confirm password</label>
            <input id="password_confirmation" name="password_confirmation" type="password" required autocomplete="new-password" :passwordrules="passwordRules" />
            <button type="submit" :disabled="processing">Save password</button>
        </Form>
        <section v-if="canManageTwoFactor">
            <h2>Two-factor authentication</h2>
            <Form v-if="!twoFactorEnabled && !twoFactorAwaitingConfirmation" action="/user/two-factor-authentication" method="post" :headers="{ 'x-csrf-token': csrf_token }">
                <button type="submit">Enable two-factor authentication</button>
            </Form>
            <div v-if="twoFactorAwaitingConfirmation">
                <p>Scan this QR code with your authenticator app, then enter the generated code.</p>
                <img v-if="twoFactorQrCode" :src="`data:image/png;base64,${twoFactorQrCode}`" alt="Two-factor authentication QR code" />
                <p v-if="twoFactorSecretKey">Setup key: <code>{{ twoFactorSecretKey }}</code></p>
                <Form action="/user/confirmed-two-factor-authentication" method="post" :headers="{ 'x-csrf-token': csrf_token }" reset-on-success>
                    <label for="code">Authentication code</label>
                    <input id="code" name="code" required inputmode="numeric" autocomplete="one-time-code" />
                    <p v-if="errors.code" role="alert">{{ errors.code }}</p>
                    <button type="submit">Confirm</button>
                </Form>
            </div>
            <div v-if="twoFactorEnabled">
                <h3>Recovery codes</h3>
                <ul><li v-for="code in twoFactorRecoveryCodes" :key="code"><code>{{ code }}</code></li></ul>
                <Form action="/user/two-factor-recovery-codes" method="post" :headers="{ 'x-csrf-token': csrf_token }">
                    <button type="submit">Regenerate recovery codes</button>
                </Form>
                <Form action="/user/two-factor-authentication" method="delete" :headers="{ 'x-csrf-token': csrf_token }">
                    <button type="submit">Disable two-factor authentication</button>
                </Form>
            </div>
        </section>
        <section v-if="canManagePasskeys">
            <h2>Passkeys</h2>
            <form @submit.prevent="registerPasskey">
                <label for="passkey-name">Passkey name</label>
                <input id="passkey-name" v-model="passkeyName" required />
                <button type="submit" :disabled="registeringPasskey || !Passkeys.isSupported()">Add passkey</button>
            </form>
            <p v-if="passkeyError" role="alert">{{ passkeyError }}</p>
            <ul>
                <li v-for="passkey in passkeys" :key="passkey.id">
                    <strong>{{ passkey.name }}</strong>
                    <span>{{ passkey.last_used_at ? ` Last used ${new Date(passkey.last_used_at * 1000).toLocaleString()}` : ' Never used' }}</span>
                    <Form :action="`/user/passkeys/${passkey.id}`" method="delete" :headers="{ 'x-csrf-token': csrf_token }">
                        <button type="submit">Delete</button>
                    </Form>
                </li>
            </ul>
        </section>
    </main>
</template>
