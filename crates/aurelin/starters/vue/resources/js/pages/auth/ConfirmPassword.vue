<script setup lang="ts">
import { Form, Head } from '@inertiajs/vue3';

defineProps<{ csrf_token: string; errors: Record<string, string> }>();
</script>

<template>
    <main>
        <Head title="Confirm password" />
        <h1>Confirm your password</h1>
        <p>This is a secure area. Confirm your password before continuing.</p>
        <Form action="/user/confirm-password" method="post" :headers="{ 'x-csrf-token': csrf_token }" :reset-on-success="['password']" v-slot="{ processing }">
            <label for="password">Password</label>
            <input id="password" name="password" type="password" required autofocus autocomplete="current-password" />
            <p v-if="errors.password" role="alert">{{ errors.password }}</p>
            <button type="submit" :disabled="processing">Confirm password</button>
        </Form>
    </main>
</template>
