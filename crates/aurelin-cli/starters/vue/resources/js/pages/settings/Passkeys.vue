<script setup lang="ts">
import { Form, Head, Link, router } from '@inertiajs/vue3';
import { Passkeys } from '@laravel/passkeys';
import { ref } from 'vue';

const props = defineProps<{
    passkeys: { id: number; name: string; last_used_at?: number; created_at: number }[];
    csrf_token: string;
}>();
const name = ref('');
const registering = ref(false);
const error = ref<string>();

const register = async () => {
    registering.value = true;
    error.value = undefined;
    try {
        Passkeys.configure({ fetch: { headers: { 'X-CSRF-TOKEN': props.csrf_token } } });
        await Passkeys.register({ name: name.value });
        router.reload({ only: ['passkeys'] });
        name.value = '';
    } catch (caught) {
        error.value = caught instanceof Error ? caught.message : 'The passkey could not be registered.';
    } finally {
        registering.value = false;
    }
};
</script>

<template>
    <main>
        <Head title="Passkeys" />
        <Link href="/dashboard">Dashboard</Link>
        <h1>Passkeys</h1>
        <form @submit.prevent="register">
            <label for="name">Passkey name</label>
            <input id="name" v-model="name" required />
            <button type="submit" :disabled="registering || !Passkeys.isSupported()">Add passkey</button>
        </form>
        <p v-if="error" role="alert">{{ error }}</p>
        <ul>
            <li v-for="passkey in passkeys" :key="passkey.id">
                <strong>{{ passkey.name }}</strong>
                <span>{{ passkey.last_used_at ? ` Last used ${new Date(passkey.last_used_at * 1000).toLocaleString()}` : ' Never used' }}</span>
                <Form :action="`/user/passkeys/${passkey.id}`" method="delete" :headers="{ 'x-csrf-token': csrf_token }">
                    <button type="submit">Delete</button>
                </Form>
            </li>
        </ul>
    </main>
</template>
