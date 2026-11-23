<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';
import { Passkeys } from '@laravel/passkeys';
import { ref } from 'vue';

const props = defineProps<{
    csrf_token: string;
    errors: Record<string, string>;
    status?: string;
    canResetPassword: boolean;
}>();
const passkeyError = ref<string>();
const verifying = ref(false);

const verifyPasskey = async () => {
    verifying.value = true;
    passkeyError.value = undefined;
    try {
        Passkeys.configure({ fetch: { headers: { 'X-CSRF-TOKEN': props.csrf_token } } });
        const response = await Passkeys.verify();
        window.location.href = response.redirect ?? '/dashboard';
    } catch (error) {
        passkeyError.value = error instanceof Error ? error.message : 'The passkey could not be verified.';
        verifying.value = false;
    }
};
</script>

<template>
    <main>
        <Head title="Log in" />
        <h1>Log in to your account</h1>
        <p v-if="status">{{ status }}</p>
        <Form action="/login" method="post" :headers="{ 'x-csrf-token': csrf_token }" :reset-on-success="['password']" v-slot="{ processing }">
            <label for="email">Email address</label>
            <input id="email" name="email" type="email" required autofocus autocomplete="email webauthn" />
            <p v-if="errors.email" role="alert">{{ errors.email }}</p>

            <label for="password">Password</label>
            <Link v-if="canResetPassword" href="/forgot-password">Forgot your password?</Link>
            <input id="password" name="password" type="password" required autocomplete="current-password" />
            <p v-if="errors.password" role="alert">{{ errors.password }}</p>

            <label for="remember">
                <input id="remember" name="remember" type="checkbox" />
                Remember me
            </label>

            <button type="submit" :disabled="processing">Log in</button>
        </Form>
        <button v-if="Passkeys.isSupported()" type="button" :disabled="verifying" @click="verifyPasskey">
            Log in with a passkey
        </button>
        <p v-if="passkeyError" role="alert">{{ passkeyError }}</p>
        <p>Don't have an account? <Link href="/register">Sign up</Link></p>
    </main>
</template>
