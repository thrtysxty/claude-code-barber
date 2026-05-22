use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

const GIT_STATUS: &str = include_str!("../benchmarks/fixtures/git_status_noisy.txt");
const CARGO_BUILD: &str = include_str!("../benchmarks/fixtures/cargo_build_noisy.txt");
const PYTEST: &str = include_str!("../benchmarks/fixtures/pytest_output.txt");
const TSC: &str = include_str!("../benchmarks/fixtures/tsc_output.txt");
const NPM_INSTALL: &str = include_str!("../benchmarks/fixtures/npm_install_noisy.txt");
const PIP_INSTALL: &str = include_str!("../benchmarks/fixtures/pip_install_noisy.txt");

fn bench_trim(c: &mut Criterion) {
    let mut group = c.benchmark_group("trim");

    for (name, input) in [
        ("git_status", GIT_STATUS),
        ("cargo_build", CARGO_BUILD),
        ("pytest", PYTEST),
        ("tsc", TSC),
        ("npm_install", NPM_INSTALL),
        ("pip_install", PIP_INSTALL),
    ] {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("compress", name), input, |b, i| {
            b.iter(|| ccb::features::trim::compress_str(i))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_trim);
criterion_main!(benches);
