<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{
    enabled: boolean;
    awaiting_confirmation: boolean;
    secret_key?: string;
    qr_code?: string;
    recovery_codes: string[];
    errors: Record<string, string>;
    csrf_token: string;
}>();
</script>

<template>
    <main>
        <Head title="Two-factor authentication" />
        <Link href="/dashboard">Dashboard</Link>
        <h1>Two-factor authentication</h1>
        <Form v-if="!enabled && !awaiting_confirmation" action="/user/two-factor-authentication" method="post" :headers="{ 'x-csrf-token': csrf_token }">
            <button type="submit">Enable two-factor authentication</button>
        </Form>
        <section v-if="awaiting_confirmation">
            <h2>Finish enabling two-factor authentication</h2>
            <p>Scan this QR code with your authenticator app, then enter the generated code.</p>
            <img v-if="qr_code" :src="`data:image/png;base64,${qr_code}`" alt="Two-factor authentication QR code" />
            <p v-if="secret_key">Setup key: <code>{{ secret_key }}</code></p>
            <Form action="/user/confirmed-two-factor-authentication" method="post" :headers="{ 'x-csrf-token': csrf_token }" reset-on-success>
                <label for="code">Authentication code</label>
                <input id="code" name="code" required inputmode="numeric" autocomplete="one-time-code" />
                <p v-if="errors.code" role="alert">{{ errors.code }}</p>
                <button type="submit">Confirm</button>
            </Form>
        </section>
        <section v-if="enabled">
            <h2>Recovery codes</h2>
            <ul><li v-for="code in recovery_codes" :key="code"><code>{{ code }}</code></li></ul>
            <Form action="/user/two-factor-recovery-codes" method="post" :headers="{ 'x-csrf-token': csrf_token }">
                <button type="submit">Regenerate recovery codes</button>
            </Form>
            <Form action="/user/two-factor-authentication" method="delete" :headers="{ 'x-csrf-token': csrf_token }">
                <button type="submit">Disable two-factor authentication</button>
            </Form>
        </section>
    </main>
</template>
