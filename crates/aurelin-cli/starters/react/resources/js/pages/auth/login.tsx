import { Form, Head, Link } from '@inertiajs/react';

type Props = {
    csrf_token: string;
    errors: Record<string, string>;
    status?: string;
};

export default function Login({ csrf_token, errors, status }: Props) {
    return (
        <main>
            <Head title="Log in" />
            <h1>Log in to your account</h1>
            {status && <p>{status}</p>}
            <Form action="/login" method="post" headers={{ 'x-csrf-token': csrf_token }} resetOnSuccess={['password']}>
                {({ processing }) => (
                    <>
                        <label htmlFor="email">Email address</label>
                        <input id="email" name="email" type="email" required autoFocus autoComplete="email" />
                        {errors.email && <p role="alert">{errors.email}</p>}

                        <label htmlFor="password">Password</label>
                        <Link href="/forgot-password">Forgot your password?</Link>
                        <input id="password" name="password" type="password" required autoComplete="current-password" />
                        {errors.password && <p role="alert">{errors.password}</p>}

                        <button type="submit" disabled={processing}>Log in</button>
                    </>
                )}
            </Form>
            <p>Don't have an account? <Link href="/register">Sign up</Link></p>
        </main>
    );
}
