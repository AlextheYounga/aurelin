import { Form, Head, Link } from '@inertiajs/react';

type Props = {
    auth: { user: { name: string; email: string; email_verified_at?: number } };
    errors: Record<string, string>;
    status?: string;
    csrf_token: string;
    mustVerifyEmail: boolean;
};

export default function ProfileSettings({ auth, errors, status, csrf_token, mustVerifyEmail }: Props) {
    const headers = { 'x-csrf-token': csrf_token };

    return (
        <main>
            <Head title="Profile settings" />
            <nav>
                <Link href="/dashboard">Dashboard</Link>
                <Link href="/settings/profile">Profile</Link>
                <Link href="/settings/security">Security</Link>
                <Link href="/settings/appearance">Appearance</Link>
            </nav>
            <h1>Profile settings</h1>
            {status === 'profile-updated' && <p>Profile updated.</p>}
            <Form action="/settings/profile" method="patch" headers={headers}>
                {({ processing }) => (
                    <>
                        <label htmlFor="name">Name</label>
                        <input id="name" name="name" defaultValue={auth.user.name} required autoComplete="name" />
                        {errors.name && <p role="alert">{errors.name}</p>}

                        <label htmlFor="email">Email address</label>
                        <input id="email" name="email" type="email" defaultValue={auth.user.email} required autoComplete="username" />
                        {errors.email && <p role="alert">{errors.email}</p>}
                        <button type="submit" disabled={processing}>Save</button>
                    </>
                )}
            </Form>

            {mustVerifyEmail && !auth.user.email_verified_at && (
                <section>
                    <p>Your email address is unverified.</p>
                    <Form action="/email/verification-notification" method="post" headers={headers}>
                        <button type="submit">Resend verification email</button>
                    </Form>
                </section>
            )}

            <section>
                <h2>Delete account</h2>
                <p>Once your account is deleted, all of its resources and data will be permanently deleted.</p>
                <Form action="/settings/profile" method="delete" headers={headers} resetOnSuccess={['password']}>
                    {({ processing }) => (
                        <>
                            <label htmlFor="delete_password">Password</label>
                            <input id="delete_password" name="password" type="password" required autoComplete="current-password" />
                            {errors.password && <p role="alert">{errors.password}</p>}
                            <button type="submit" disabled={processing}>Delete account</button>
                        </>
                    )}
                </Form>
            </section>
        </main>
    );
}
