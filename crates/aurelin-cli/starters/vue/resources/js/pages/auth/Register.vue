<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{ csrf_token: string; errors: Record<string, string>; passwordRules: string }>();
</script>

<template>
    <main>
        <Head title="Register" />
        <h1>Create an account</h1>
        <Form action="/register" method="post" :headers="{ 'x-csrf-token': csrf_token }" :reset-on-success="['password', 'password_confirmation']" v-slot="{ processing }">
            <label for="name">Name</label>
            <input id="name" name="name" required autofocus autocomplete="name" />
            <p v-if="errors.name" role="alert">{{ errors.name }}</p>

            <label for="email">Email address</label>
            <input id="email" name="email" type="email" required autocomplete="email" />
            <p v-if="errors.email" role="alert">{{ errors.email }}</p>

            <label for="password">Password</label>
            <input id="password" name="password" type="password" required autocomplete="new-password" :passwordrules="passwordRules" />
            <p v-if="errors.password" role="alert">{{ errors.password }}</p>

            <label for="password_confirmation">Confirm password</label>
            <input id="password_confirmation" name="password_confirmation" type="password" required autocomplete="new-password" :passwordrules="passwordRules" />

            <button type="submit" :disabled="processing">Create account</button>
        </Form>
        <p>Already have an account? <Link href="/login">Log in</Link></p>
    </main>
</template>
