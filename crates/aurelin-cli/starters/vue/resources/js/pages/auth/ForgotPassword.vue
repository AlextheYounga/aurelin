<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{ csrf_token: string; errors: Record<string, string>; status?: string }>();
</script>

<template>
    <main>
        <Head title="Forgot password" />
        <h1>Forgot your password?</h1>
        <p>Enter your email address and we will send you a password reset link.</p>
        <p v-if="status">{{ status }}</p>
        <Form action="/forgot-password" method="post" :headers="{ 'x-csrf-token': csrf_token }" v-slot="{ processing }">
            <label for="email">Email address</label>
            <input id="email" name="email" type="email" required autofocus autocomplete="email" />
            <p v-if="errors.email" role="alert">{{ errors.email }}</p>
            <button type="submit" :disabled="processing">Email password reset link</button>
        </Form>
        <Link href="/login">Back to log in</Link>
    </main>
</template>
