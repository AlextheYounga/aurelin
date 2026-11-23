<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{ csrf_token: string; status?: string }>();
</script>

<template>
    <main>
        <Head title="Email verification" />
        <h1>Verify your email</h1>
        <p>Verify your email address by following the link sent to you.</p>
        <p v-if="status === 'verification-link-sent'">A new verification link has been sent.</p>
        <Form action="/email/verification-notification" method="post" :headers="{ 'x-csrf-token': csrf_token }" v-slot="{ processing }">
            <button type="submit" :disabled="processing">Resend verification email</button>
        </Form>
        <Link href="/">Return home</Link>
    </main>
</template>
