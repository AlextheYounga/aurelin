import { Form, Head, Link } from '@inertiajs/react';

type Props = {
    csrf_token: string;
    status?: string;
};

export default function VerifyEmail({ csrf_token, status }: Props) {
    return (
        <main>
            <Head title="Email verification" />
            <h1>Verify your email</h1>
            <p>Verify your email address by following the link sent to you.</p>
            {status === 'verification-link-sent' && <p>A new verification link has been sent.</p>}
            <Form action="/email/verification-notification" method="post" headers={{ 'x-csrf-token': csrf_token }}>
                {({ processing }) => <button type="submit" disabled={processing}>Resend verification email</button>}
            </Form>
            <Link href="/">Return home</Link>
        </main>
    );
}
