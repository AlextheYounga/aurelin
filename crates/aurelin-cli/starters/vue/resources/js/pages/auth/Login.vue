<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{ csrf_token: string; errors: Record<string, string>; status?: string }>();
</script>

<template>
    <main>
        <Head title="Log in" />
        <h1>Log in to your account</h1>
        <p v-if="status">{{ status }}</p>
        <Form action="/login" method="post" :headers="{ 'x-csrf-token': csrf_token }" :reset-on-success="['password']" v-slot="{ processing }">
            <label for="email">Email address</label>
            <input id="email" name="email" type="email" required autofocus autocomplete="email" />
            <p v-if="errors.email" role="alert">{{ errors.email }}</p>

            <label for="password">Password</label>
            <Link href="/forgot-password">Forgot your password?</Link>
            <input id="password" name="password" type="password" required autocomplete="current-password" />
            <p v-if="errors.password" role="alert">{{ errors.password }}</p>

            <button type="submit" :disabled="processing">Log in</button>
        </Form>
        <p>Don't have an account? <Link href="/register">Sign up</Link></p>
    </main>
</template>
