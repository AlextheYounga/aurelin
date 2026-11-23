<script setup lang="ts">
import { Form, Head, Link } from '@inertiajs/vue3';
import './welcome.css';

defineProps<{
    message: string;
    auth: { user: { id: number; name: string; email: string } | null };
    csrf_token: string;
}>();
</script>

<template>
    <main class="welcome-page">
        <Head title="Welcome" />

        <div class="welcome-glow welcome-glow--one" />
        <div class="welcome-glow welcome-glow--two" />

        <header class="welcome-header">
            <Link class="welcome-brand" href="/" aria-label="Aurelin home">
                <span class="welcome-mark">
                    <svg aria-hidden="true" viewBox="0 0 32 32">
                        <path d="M16 3 28 9.5v13L16 29 4 22.5v-13L16 3Z" />
                        <path d="m10.5 20.5 5.5-10 5.5 10M12.5 17h7" />
                    </svg>
                </span>
                <span>Aurelin</span>
            </Link>

            <nav class="welcome-nav" aria-label="Account navigation">
                <template v-if="auth.user">
                    <Link class="welcome-link" href="/dashboard">Dashboard</Link>
                    <Form action="/logout" method="post" :headers="{ 'x-csrf-token': csrf_token }">
                        <button class="welcome-button welcome-button--small" type="submit">Log out</button>
                    </Form>
                </template>
                <template v-else>
                    <Link class="welcome-link" href="/login">Log in</Link>
                    <Link class="welcome-button welcome-button--small" href="/register">Get started</Link>
                </template>
            </nav>
        </header>

        <section class="welcome-hero">
            <div class="welcome-copy">
                <p class="welcome-eyebrow"><span /> Rust-powered. Developer focused.</p>
                <h1>{{ message }}</h1>
                <p class="welcome-lead">
                    Your Vue application is connected through Inertia 3. Everything is in place—now make it yours.
                </p>
                <div class="welcome-actions">
                    <Link class="welcome-button" :href="auth.user ? '/dashboard' : '/register'">
                        {{ auth.user ? 'Open dashboard' : 'Start building' }}
                        <span aria-hidden="true">→</span>
                    </Link>
                    <Link v-if="!auth.user" class="welcome-text-link" href="/login">I already have an account</Link>
                </div>
            </div>

            <div class="welcome-preview" aria-label="Application status">
                <div class="welcome-preview__bar">
                    <div class="welcome-window-controls" aria-hidden="true"><span /><span /><span /></div>
                    <span>your-app</span>
                    <span class="welcome-preview__status"><i /> Ready</span>
                </div>
                <div class="welcome-preview__body">
                    <p class="welcome-preview__label">REQUEST LIFECYCLE</p>
                    <div class="welcome-flow">
                        <div><span>01</span><strong>Browser</strong><small>Client request</small></div>
                        <i>→</i>
                        <div><span>02</span><strong>Inertia</strong><small>Vue page</small></div>
                        <i>→</i>
                        <div><span>03</span><strong>Aurelin</strong><small>Rust response</small></div>
                    </div>
                    <div class="welcome-terminal">
                        <p><span>$</span> cargo aurelin serve</p>
                        <p><i>✓</i> Application ready at <strong>localhost:3000</strong></p>
                        <span class="welcome-cursor" />
                    </div>
                </div>
            </div>
        </section>

        <footer class="welcome-footer">
            <p>Built for the web, with the confidence of Rust.</p>
            <div><span>INERTIA 3</span><span>VUE 3</span><span>TYPE SAFE</span></div>
        </footer>
    </main>
</template>
