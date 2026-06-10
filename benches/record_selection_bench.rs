use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shotmd::ui::{MonitorKey, Selection};

fn bench_record_to_webp(c: &mut Criterion) {
    let selection = Selection {
        x: 0,
        y: 0,
        width: 512,
        height: 512,
        monitor: MonitorKey {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
    };

    let mut group = c.benchmark_group("record_to_webp");
    group.sample_size(10);

    group.bench_function("encode", |b| {
        b.iter(|| {
            let _ = black_box(shotmd::commands::record_command::record_to_webp(
                black_box(&selection),
                black_box(15),
            ));
        })
    });

    group.finish();
}

criterion_group!(benches, bench_record_to_webp);
criterion_main!(benches);
