<script setup lang="ts">
import { Head, Link } from '@inertiajs/vue3';
import { ref } from 'vue';
import { type Appearance, storedAppearance, updateAppearance } from '../../appearance';

const appearance = ref<Appearance>(storedAppearance());

const update = (value: Appearance) => {
    appearance.value = value;
    updateAppearance(value);
};
</script>

<template>
    <main>
        <Head title="Appearance settings" />
        <nav>
            <Link href="/dashboard">Dashboard</Link>
            <Link href="/settings/profile">Profile</Link>
            <Link href="/settings/security">Security</Link>
            <Link href="/settings/appearance">Appearance</Link>
        </nav>
        <h1>Appearance settings</h1>
        <fieldset>
            <legend>Theme</legend>
            <label v-for="value in (['light', 'dark', 'system'] as const)" :key="value">
                <input type="radio" name="appearance" :value="value" :checked="appearance === value" @change="update(value)" />
                {{ value.charAt(0).toUpperCase() + value.slice(1) }}
            </label>
        </fieldset>
    </main>
</template>
