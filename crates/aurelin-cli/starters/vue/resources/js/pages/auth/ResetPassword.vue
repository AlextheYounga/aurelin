<script setup lang="ts">
import { Form, Head } from '@inertiajs/vue3';

defineProps<{
    csrf_token: string;
    email: string;
    token: string;
    errors: Record<string, string>;
}>();
</script>

<template>
    <main>
        <Head title="Reset password" />
        <h1>Reset password</h1>
        <Form :action="`/reset-password/${token}?email=${encodeURIComponent(email)}`" method="post" :headers="{ 'x-csrf-token': csrf_token }" v-slot="{ processing }">
            <label for="email">Email address</label>
            <input id="email" type="email" :value="email" readonly autocomplete="email" />
            <p v-if="errors.email" role="alert">{{ errors.email }}</p>
            <label for="password">Password</label>
            <input id="password" name="password" type="password" required autofocus autocomplete="new-password" />
            <p v-if="errors.password" role="alert">{{ errors.password }}</p>
            <label for="password_confirmation">Confirm password</label>
            <input id="password_confirmation" name="password_confirmation" type="password" required autocomplete="new-password" />
            <button type="submit" :disabled="processing">Reset password</button>
        </Form>
    </main>
</template>
