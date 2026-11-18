<script setup lang="ts">
import { Form, Head } from '@inertiajs/vue3';

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
        <Form action="/logout" method="post" :headers="{ 'x-csrf-token': csrf_token }">
            <button type="submit">Log out</button>
        </Form>
    </main>
</template>
