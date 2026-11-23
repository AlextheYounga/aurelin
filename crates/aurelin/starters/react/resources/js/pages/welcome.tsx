import { Form, Head, Link } from '@inertiajs/react';
import './welcome.css';

type User = { id: number; name: string; email: string };

type Props = {
    message: string;
    auth: { user: User | null };
    csrf_token: string;
};

function AurelinMark() {
    return (
        <svg aria-hidden="true" viewBox="0 0 32 32">
            <path d="M16 3 28 9.5v13L16 29 4 22.5v-13L16 3Z" />
            <path d="m10.5 20.5 5.5-10 5.5 10M12.5 17h7" />
        </svg>
    );
}

export default function Welcome({ message, auth, csrf_token }: Props) {
    return (
        <main className="welcome-page">
            <Head title="Welcome" />

            <div className="welcome-glow welcome-glow--one" />
            <div className="welcome-glow welcome-glow--two" />

            <header className="welcome-header">
                <Link className="welcome-brand" href="/" aria-label="Aurelin home">
                    <span className="welcome-mark"><AurelinMark /></span>
                    <span>Aurelin</span>
                </Link>

                <nav className="welcome-nav" aria-label="Account navigation">
                    {auth.user ? (
                        <>
                            <Link className="welcome-link" href="/dashboard">Dashboard</Link>
                            <Form action="/logout" method="post" headers={{ 'x-csrf-token': csrf_token }}>
                                <button className="welcome-button welcome-button--small" type="submit">Log out</button>
                            </Form>
                        </>
                    ) : (
                        <>
                            <Link className="welcome-link" href="/login">Log in</Link>
                            <Link className="welcome-button welcome-button--small" href="/register">Get started</Link>
                        </>
                    )}
                </nav>
            </header>

            <section className="welcome-hero">
                <div className="welcome-copy">
                    <p className="welcome-eyebrow"><span /> Rust-powered. Developer focused.</p>
                    <h1>{message}</h1>
                    <p className="welcome-lead">
                        Your React application is connected through Inertia 3. Everything is in place—now make it yours.
                    </p>
                    <div className="welcome-actions">
                        <Link className="welcome-button" href={auth.user ? '/dashboard' : '/register'}>
                            {auth.user ? 'Open dashboard' : 'Start building'}
                            <span aria-hidden="true">→</span>
                        </Link>
                        {!auth.user && <Link className="welcome-text-link" href="/login">I already have an account</Link>}
                    </div>
                </div>

                <div className="welcome-preview" aria-label="Application status">
                    <div className="welcome-preview__bar">
                        <div className="welcome-window-controls" aria-hidden="true"><span /><span /><span /></div>
                        <span>your-app</span>
                        <span className="welcome-preview__status"><i /> Ready</span>
                    </div>
                    <div className="welcome-preview__body">
                        <p className="welcome-preview__label">REQUEST LIFECYCLE</p>
                        <div className="welcome-flow">
                            <div><span>01</span><strong>Browser</strong><small>Client request</small></div>
                            <i>→</i>
                            <div><span>02</span><strong>Inertia</strong><small>React page</small></div>
                            <i>→</i>
                            <div><span>03</span><strong>Aurelin</strong><small>Rust response</small></div>
                        </div>
                        <div className="welcome-terminal">
                            <p><span>$</span> cargo aurelin serve</p>
                            <p><i>✓</i> Application ready at <strong>localhost:3000</strong></p>
                            <span className="welcome-cursor" />
                        </div>
                    </div>
                </div>
            </section>

            <footer className="welcome-footer">
                <p>Built for the web, with the confidence of Rust.</p>
                <div><span>INERTIA 3</span><span>REACT 19</span><span>TYPE SAFE</span></div>
            </footer>
        </main>
    );
}
