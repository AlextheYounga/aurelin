import { Form, Head } from '@inertiajs/react';

type Props = {
    csrf_token: string;
    email: string;
    token: string;
    errors: Record<string, string>;
};

export default function ResetPassword({ csrf_token, email, token, errors }: Props) {
    return (
        <main>
            <Head title="Reset password" />
            <h1>Reset password</h1>
            <Form action={`/reset-password/${token}?email=${encodeURIComponent(email)}`} method="post" headers={{ 'x-csrf-token': csrf_token }}>
                {({ processing }) => (
                    <>
                        <label htmlFor="email">Email address</label>
                        <input id="email" type="email" value={email} readOnly autoComplete="email" />
                        {errors.email && <p role="alert">{errors.email}</p>}
                        <label htmlFor="password">Password</label>
                        <input id="password" name="password" type="password" required autoFocus autoComplete="new-password" />
                        {errors.password && <p role="alert">{errors.password}</p>}
                        <label htmlFor="password_confirmation">Confirm password</label>
                        <input id="password_confirmation" name="password_confirmation" type="password" required autoComplete="new-password" />
                        <button type="submit" disabled={processing}>Reset password</button>
                    </>
                )}
            </Form>
        </main>
    );
}
