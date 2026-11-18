import { Form, Head, Link } from '@inertiajs/react';

type Props = {
    csrf_token: string;
    errors: Record<string, string>;
    status?: string;
};

export default function ForgotPassword({ csrf_token, errors, status }: Props) {
    return (
        <main>
            <Head title="Forgot password" />
            <h1>Forgot your password?</h1>
            <p>Enter your email address and we will send you a password reset link.</p>
            {status && <p>{status}</p>}
            <Form action="/forgot-password" method="post" headers={{ 'x-csrf-token': csrf_token }}>
                {({ processing }) => (
                    <>
                        <label htmlFor="email">Email address</label>
                        <input id="email" name="email" type="email" required autoFocus autoComplete="email" />
                        {errors.email && <p role="alert">{errors.email}</p>}
                        <button type="submit" disabled={processing}>Email password reset link</button>
                    </>
                )}
            </Form>
            <Link href="/login">Back to log in</Link>
        </main>
    );
}
