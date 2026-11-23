<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';

defineProps<{
    auth: { user: { name: string; email: string; email_verified_at?: number } };
    errors: Record<string, string>;
    status?: string;
    csrf_token: string;
    mustVerifyEmail: boolean;
}>();
</script>

<template>
    <main>
        <Head title="Profile settings" />
        <nav>
            <Link href="/dashboard">Dashboard</Link>
            <Link href="/settings/profile">Profile</Link>
            <Link href="/settings/security">Security</Link>
            <Link href="/settings/appearance">Appearance</Link>
        </nav>
        <h1>Profile settings</h1>
        <p v-if="status === 'profile-updated'">Profile updated.</p>
        <Form action="/settings/profile" method="patch" :headers="{ 'x-csrf-token': csrf_token }" v-slot="{ processing }">
            <label for="name">Name</label>
            <input id="name" name="name" :value="auth.user.name" required autocomplete="name" />
            <p v-if="errors.name" role="alert">{{ errors.name }}</p>

            <label for="email">Email address</label>
            <input id="email" name="email" type="email" :value="auth.user.email" required autocomplete="username" />
            <p v-if="errors.email" role="alert">{{ errors.email }}</p>
            <button type="submit" :disabled="processing">Save</button>
        </Form>

        <section v-if="mustVerifyEmail && !auth.user.email_verified_at">
            <p>Your email address is unverified.</p>
            <Form action="/email/verification-notification" method="post" :headers="{ 'x-csrf-token': csrf_token }">
                <button type="submit">Resend verification email</button>
            </Form>
        </section>

        <section>
            <h2>Delete account</h2>
            <p>Once your account is deleted, all of its resources and data will be permanently deleted.</p>
            <Form action="/settings/profile" method="delete" :headers="{ 'x-csrf-token': csrf_token }" :reset-on-success="['password']" v-slot="{ processing }">
                <label for="delete_password">Password</label>
                <input id="delete_password" name="password" type="password" required autocomplete="current-password" />
                <p v-if="errors.password" role="alert">{{ errors.password }}</p>
                <button type="submit" :disabled="processing">Delete account</button>
            </Form>
        </section>
    </main>
</template>
