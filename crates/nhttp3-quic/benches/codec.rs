use bytes::{Bytes, BytesMut};
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use nhttp3_core::VarInt;
use nhttp3_quic::frame::Frame;
use nhttp3_quic::packet::Header;
use nhttp3_quic::transport::TransportParams;

fn varint_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("varint");

    group.bench_function("encode_1byte", |b| {
        let v = VarInt::from_u32(37);
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(8);
            black_box(&v).encode(&mut buf);
            black_box(buf);
        });
    });

    group.bench_function("encode_8byte", |b| {
        let v = VarInt::try_from(151_288_809_941_952_652u64).unwrap();
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(8);
            black_box(&v).encode(&mut buf);
            black_box(buf);
        });
    });

    group.bench_function("decode_mixed", |b| {
        let mut encoded = BytesMut::new();
        for val in [37u64, 15293, 494_878_333, 151_288_809_941_952_652] {
            VarInt::try_from(val).unwrap().encode(&mut encoded);
        }
        let data = encoded.freeze();
        b.iter(|| {
            let mut buf = data.clone();
            for _ in 0..4 {
                black_box(VarInt::decode(&mut buf).unwrap());
            }
        });
    });

    group.finish();
}

fn frame_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame");

    let stream_frame = Frame::Stream {
        stream_id: VarInt::from_u32(4),
        offset: Some(VarInt::from_u32(1024)),
        data: vec![0xab; 1200],
        fin: false,
    };

    let ack_frame = Frame::Ack {
        largest_ack: VarInt::from_u32(100),
        ack_delay: VarInt::from_u32(25),
        first_ack_range: VarInt::from_u32(10),
        ack_ranges: vec![],
        ecn: None,
    };

    group.throughput(Throughput::Bytes(1200));
    group.bench_function("encode_stream_1200b", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(1300);
            black_box(&stream_frame).encode(&mut buf);
            black_box(buf);
        });
    });

    group.bench_function("parse_stream_1200b", |b| {
        let mut buf = BytesMut::new();
        stream_frame.encode(&mut buf);
        let data = buf.freeze();
        b.iter(|| {
            let mut d = data.clone();
            black_box(Frame::parse(&mut d).unwrap());
        });
    });

    group.bench_function("encode_ack", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(32);
            black_box(&ack_frame).encode(&mut buf);
            black_box(buf);
        });
    });

    group.bench_function("parse_ack", |b| {
        let mut buf = BytesMut::new();
        ack_frame.encode(&mut buf);
        let data = buf.freeze();
        b.iter(|| {
            let mut d = data.clone();
            black_box(Frame::parse(&mut d).unwrap());
        });
    });

    group.finish();
}

fn packet_header_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_header");

    let initial = Bytes::from(vec![
        0xc0, 0x00, 0x00, 0x00, 0x01, 0x08, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x00, 0x00, 0x10,
    ]);

    let short = Bytes::from(vec![
        0x40, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    ]);

    group.bench_function("parse_initial", |b| {
        b.iter(|| {
            let mut buf = initial.clone();
            black_box(Header::parse(&mut buf, 0).unwrap());
        });
    });

    group.bench_function("parse_short", |b| {
        b.iter(|| {
            let mut buf = short.clone();
            black_box(Header::parse(&mut buf, 8).unwrap());
        });
    });

    group.finish();
}

fn transport_params_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("transport_params");

    let params = TransportParams {
        initial_max_data: 10_000_000,
        initial_max_streams_bidi: 100,
        ..Default::default()
    };

    group.bench_function("encode", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(256);
            black_box(&params).encode(&mut buf);
            black_box(buf);
        });
    });

    group.bench_function("decode", |b| {
        let mut buf = BytesMut::new();
        params.encode(&mut buf);
        let data = buf.freeze();
        b.iter(|| {
            let mut d = data.clone();
            black_box(TransportParams::decode(&mut d).unwrap());
        });
    });

    group.finish();
}

fn qpack_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("qpack");

    let encoder = nhttp3_qpack::Encoder::new(0);
    let decoder = nhttp3_qpack::Decoder::new(0);

    let request_headers = vec![
        nhttp3_qpack::HeaderField::new(":method", "GET"),
        nhttp3_qpack::HeaderField::new(":path", "/index.html"),
        nhttp3_qpack::HeaderField::new(":scheme", "https"),
        nhttp3_qpack::HeaderField::new(":authority", "example.com"),
        nhttp3_qpack::HeaderField::new("accept", "text/html"),
        nhttp3_qpack::HeaderField::new("user-agent", "nhttp3/0.1"),
    ];

    group.bench_function("encode_request_6h", |b| {
        b.iter(|| {
            black_box(encoder.encode_header_block(black_box(&request_headers)));
        });
    });

    let encoded = encoder.encode_header_block(&request_headers);

    group.bench_function("decode_request_6h", |b| {
        b.iter(|| {
            black_box(decoder.decode_header_block(black_box(&encoded)).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    varint_benchmarks,
    frame_benchmarks,
    packet_header_benchmarks,
    transport_params_benchmarks,
    qpack_benchmarks,
);
criterion_main!(benches);
