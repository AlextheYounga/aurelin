import { Form, Head, Link } from '@inertiajs/react';

type User = { id: number; name: string; email: string };

export default function Welcome({
    message,
    auth,
    csrf_token,
}: {
    message: string;
    auth: { user: User | null };
    csrf_token: string;
}) {
    return (
        <main>
            <Head title="Welcome" />
            <h1>{message}</h1>
            <p>Your React application is connected through Inertia 3.</p>
            {auth.user ? (
                <nav>
                    <Link href="/dashboard">Dashboard</Link>{' '}
                    <Form action="/logout" method="post" headers={{ 'x-csrf-token': csrf_token }}>
                        <button type="submit">Log out</button>
                    </Form>
                </nav>
            ) : (
                <nav>
                    <Link href="/login">Log in</Link>{' '}
                    <Link href="/register">Register</Link>
                </nav>
            )}
        </main>
    );
}
