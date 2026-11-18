import { Form, Head, Link } from '@inertiajs/react';

type Props = {
    csrf_token: string;
    errors: Record<string, string>;
};

export default function Register({ csrf_token, errors }: Props) {
    return (
        <main>
            <Head title="Register" />
            <h1>Create an account</h1>
            <Form action="/register" method="post" headers={{ 'x-csrf-token': csrf_token }} resetOnSuccess={['password', 'password_confirmation']}>
                {({ processing }) => (
                    <>
                        <label htmlFor="name">Name</label>
                        <input id="name" name="name" required autoFocus autoComplete="name" />
                        {errors.name && <p role="alert">{errors.name}</p>}

                        <label htmlFor="email">Email address</label>
                        <input id="email" name="email" type="email" required autoComplete="email" />
                        {errors.email && <p role="alert">{errors.email}</p>}

                        <label htmlFor="password">Password</label>
                        <input id="password" name="password" type="password" required autoComplete="new-password" />
                        {errors.password && <p role="alert">{errors.password}</p>}

                        <label htmlFor="password_confirmation">Confirm password</label>
                        <input id="password_confirmation" name="password_confirmation" type="password" required autoComplete="new-password" />

                        <button type="submit" disabled={processing}>Create account</button>
                    </>
                )}
            </Form>
            <p>Already have an account? <Link href="/login">Log in</Link></p>
        </main>
    );
}
