import React from 'react';
import clsx from 'clsx';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import Link from '@docusaurus/Link';

function HomepageHeader() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary')} style={{minHeight: '60vh', display: 'flex', alignItems: 'center'}}>
      <div className="container" style={{textAlign: 'center'}}>
        <Heading as="h1" className="hero__title">
          {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div style={{display: 'flex', gap: '1rem', justifyContent: 'center', marginTop: '2rem'}}>
          <Link className="button button--secondary button--lg" to="/docs">
            Get Started
          </Link>
          <Link className="button button--outline button--lg" style={{color: 'white', borderColor: 'white'}} href="https://traits.build/docs/api">
            API Reference
          </Link>
          <Link className="button button--outline button--lg" style={{color: 'white', borderColor: 'white'}} href="https://github.com/kilian-ai/traits.build">
            GitHub
          </Link>
        </div>
      </div>
    </header>
  );
}

const features = [
  {
    title: 'Traits All The Way Down',
    description: 'The kernel itself is built from traits. HTTP serving, config loading, the registry — all traits. No special framework code.',
  },
  {
    title: 'Single Binary',
    description: '28 traits compile into one ~2 MB executable. Zero runtime dependencies. Deploy anywhere Rust compiles.',
  },
  {
    title: 'Interface System',
    description: 'Typed dependency injection via .trait.toml. Declare requires, wire bindings. Swap implementations without changing callers.',
  },
];

function Feature({title, description}) {
  return (
    <div className={clsx('col col--4')} style={{padding: '1rem'}}>
      <div style={{textAlign: 'center', padding: '1rem'}}>
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function Home() {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout title={siteConfig.title} description="Documentation for traits.build — a composable function kernel in pure Rust">
      <HomepageHeader />
      <main>
        <section style={{padding: '4rem 0'}}>
          <div className="container">
            <div className="row">
              {features.map((props, idx) => (
                <Feature key={idx} {...props} />
              ))}
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
