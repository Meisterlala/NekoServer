use criterion::{criterion_group, criterion_main, Criterion};

use criterion::BenchmarkId;
use neko_server::*;

fn generate_image(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate_image");

    let func = CountImage::create_image;

    let values = [
        4538,
        773,
        3371,
        5850,
        1297,
        10123000,
        414142456,
        15416879521,
        1564135,
        698241574,
        51235124213,
    ];

    for img_count in values.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(img_count), img_count, |b, &size| {
            b.iter(|| func(size))
        });
    }
}

criterion_group!(benches, generate_image);
criterion_main!(benches);
