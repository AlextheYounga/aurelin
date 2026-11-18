<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{
    message: string;
    auth: { user: { id: number; name: string; email: string } | null };
    csrf_token: string;
}>();
</script>

<template>
    <main>
        <Head title="Welcome" />
        <h1>{{ message }}</h1>
        <p>Your Vue application is connected through Inertia 3.</p>
        <nav v-if="auth.user">
            <Link href="/dashboard">Dashboard</Link>
            <Form action="/logout" method="post" :headers="{ 'x-csrf-token': csrf_token }">
                <button type="submit">Log out</button>
            </Form>
        </nav>
        <nav v-else>
            <Link href="/login">Log in</Link>
            <Link href="/register">Register</Link>
        </nav>
    </main>
</template>
