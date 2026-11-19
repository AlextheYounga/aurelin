<script setup lang="ts">
import { Form, Head } from '@inertiajs/vue3';

defineProps<{
    csrf_token: string;
    email: string;
    token: string;
    errors: Record<string, string>;
    passwordRules: string;
}>();
</script>

<template>
    <main>
        <Head title="Reset password" />
        <h1>Reset password</h1>
        <Form action="/reset-password" method="post" :headers="{ 'x-csrf-token': csrf_token }" v-slot="{ processing }">
            <input type="hidden" name="token" :value="token" />
            <label for="email">Email address</label>
            <input id="email" name="email" type="email" :value="email" readonly autocomplete="email" />
            <p v-if="errors.email" role="alert">{{ errors.email }}</p>
            <label for="password">Password</label>
            <input id="password" name="password" type="password" required autofocus autocomplete="new-password" :passwordrules="passwordRules" />
            <p v-if="errors.password" role="alert">{{ errors.password }}</p>
            <label for="password_confirmation">Confirm password</label>
            <input id="password_confirmation" name="password_confirmation" type="password" required autocomplete="new-password" :passwordrules="passwordRules" />
            <button type="submit" :disabled="processing">Reset password</button>
        </Form>
    </main>
</template>
