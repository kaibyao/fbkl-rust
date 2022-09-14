use criterion::{criterion_group, criterion_main, Criterion};
use fbkl_auth::{generate_password_hash, verify_password_against_hash};

static PASSWORD: &str = "this is a test password";

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("password generation", |bencher| {
        bencher.iter(|| generate_password_hash(PASSWORD))
    });

    let hash = generate_password_hash(PASSWORD).unwrap();
    c.bench_function("password verification", |bencher| {
        bencher.iter(|| verify_password_against_hash(PASSWORD, &hash))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
