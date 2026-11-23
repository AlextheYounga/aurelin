<script setup lang="ts">
import { Form, Head } from '@inertiajs/vue3';
import { ref } from 'vue';

defineProps<{ csrf_token: string; errors: Record<string, string> }>();
const useRecoveryCode = ref(false);
</script>

<template>
    <main>
        <Head title="Two-factor authentication" />
        <h1>Two-factor authentication</h1>
        <p>{{ useRecoveryCode ? 'Enter one of your emergency recovery codes.' : 'Enter the authentication code from your authenticator app.' }}</p>
        <Form action="/two-factor-challenge" method="post" :headers="{ 'x-csrf-token': csrf_token }" reset-on-success v-slot="{ processing }">
            <template v-if="useRecoveryCode">
                <label for="recovery_code">Recovery code</label>
                <input id="recovery_code" name="recovery_code" required autofocus autocomplete="one-time-code" />
            </template>
            <template v-else>
                <label for="code">Authentication code</label>
                <input id="code" name="code" inputmode="numeric" required autofocus autocomplete="one-time-code" />
            </template>
            <p v-if="errors.code" role="alert">{{ errors.code }}</p>
            <button type="submit" :disabled="processing">Continue</button>
        </Form>
        <button type="button" @click="useRecoveryCode = !useRecoveryCode">
            {{ useRecoveryCode ? 'Use an authentication code' : 'Use a recovery code' }}
        </button>
    </main>
</template>
