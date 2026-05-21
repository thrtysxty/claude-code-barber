use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// Sample payloads representing real Claude Code noise
const GIT_STATUS_NOISY: &str = include_str!("../benchmarks/fixtures/git_status_noisy.txt");
const PYTEST_OUTPUT: &str = include_str!("../benchmarks/fixtures/pytest_output.txt");
const TSC_OUTPUT: &str = include_str!("../benchmarks/fixtures/tsc_output.txt");

fn bench_trim(c: &mut Criterion) {
    let mut group = c.benchmark_group("trim");

    for (name, input) in [
        ("git_status", GIT_STATUS_NOISY),
        ("pytest", PYTEST_OUTPUT),
        ("tsc", TSC_OUTPUT),
    ] {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("compress", name),
            input,
            |b, i| b.iter(|| ccb::features::trim::compress_str(i)),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_trim);
criterion_main!(benches);
