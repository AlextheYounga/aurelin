import { Form, Head, Link } from '@inertiajs/react';

type Props = {
    auth: { user: { id: number; name: string; email: string } };
    csrf_token: string;
};

export default function Dashboard({ auth, csrf_token }: Props) {
    return (
        <main>
            <Head title="Dashboard" />
            <h1>Dashboard</h1>
            <p>Welcome, {auth.user.name}.</p>
            <Link href="/settings/two-factor">Two-factor authentication</Link>
            <Link href="/settings/passkeys">Passkeys</Link>
            <Form action="/logout" method="post" headers={{ 'x-csrf-token': csrf_token }}>
                <button type="submit">Log out</button>
            </Form>
        </main>
    );
}
