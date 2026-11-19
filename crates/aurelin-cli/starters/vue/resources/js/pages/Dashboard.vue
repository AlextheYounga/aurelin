<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{
    auth: { user: { id: number; name: string; email: string } };
    csrf_token: string;
}>();
</script>

<template>
    <main>
        <Head title="Dashboard" />
        <h1>Dashboard</h1>
        <p>Welcome, {{ auth.user.name }}.</p>
        <Link href="/settings/profile">Settings</Link>
        <Link href="/settings/two-factor">Two-factor authentication</Link>
        <Link href="/settings/passkeys">Passkeys</Link>
        <Form action="/logout" method="post" :headers="{ 'x-csrf-token': csrf_token }">
            <button type="submit">Log out</button>
        </Form>
    </main>
</template>
