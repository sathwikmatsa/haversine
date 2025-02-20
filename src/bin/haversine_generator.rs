use core::fmt;
use std::{
    fs::File,
    io::{BufWriter, Write},
};

use byteorder::{LittleEndian, WriteBytesExt};
use clap::{Parser, ValueEnum};
use haversine::{
    reference_haversine, HaversineData, HaversineDataPoint, EARTH_RADIUS, X_HIGH, X_LOW, Y_HIGH,
    Y_LOW,
};
use rand::{
    distributions::{Distribution, Uniform},
    Rng, SeedableRng,
};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use serde_json::ser::PrettyFormatter;

#[derive(Clone, Copy, ValueEnum)]
enum HaversineDist {
    Uniform,
    Cluster,
}

impl fmt::Display for HaversineDist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cluster => write!(f, "cluster"),
            Self::Uniform => write!(f, "uniform"),
        }
    }
}

#[derive(Parser)]
struct Arguments {
    #[arg(name = "uniform/cluster")]
    dist: HaversineDist,
    #[arg(name = "random seed")]
    seed: u64,
    #[arg(name = "number of coordinate pairs to generate")]
    pair_count: usize,
}

fn generate_haversine_data_uniform(n: usize, seed: u64) -> HaversineData {
    let mut pairs: Vec<HaversineDataPoint> = Vec::with_capacity(n);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let uniform_x = Uniform::new_inclusive(X_LOW, X_HIGH);
    let uniform_y = Uniform::new_inclusive(Y_LOW, Y_HIGH);
    for _ in 0..n {
        pairs.push(HaversineDataPoint {
            x0: uniform_x.sample(&mut rng),
            y0: uniform_y.sample(&mut rng),
            x1: uniform_x.sample(&mut rng),
            y1: uniform_y.sample(&mut rng),
        });
    }
    HaversineData { pairs }
}

fn distribution_clusters(
    start: f64,
    end: f64,
    parts: usize,
    rng: &mut impl Rng,
) -> Vec<Uniform<f64>> {
    let mut breakpoints: Vec<f64> = (0..parts - 1).map(|_| rng.gen_range(start..end)).collect();
    breakpoints.sort_by(|a, b| a.partial_cmp(b).unwrap());
    breakpoints.insert(0, start);
    breakpoints.push(end);

    breakpoints
        .windows(2)
        .map(|range| Uniform::new(range[0], range[1]))
        .collect()
}

fn generate_haversine_data_cluster(n: usize, seed: u64) -> HaversineData {
    let mut pairs: Vec<HaversineDataPoint> = Vec::with_capacity(n);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let cluster_size: usize = match n {
        0..=1000 => 4,
        1001..=100_000 => 8,
        100_001..=1_000_000 => 16,
        1_000_001..=10_000_000 => 32,
        _ => 64,
    };
    debug_assert!(cluster_size.is_power_of_two());
    #[allow(clippy::pedantic)]
    let parts = (cluster_size as f64).sqrt() as usize;

    let x_clusters = distribution_clusters(X_LOW, X_HIGH, parts, &mut rng);
    let y_clusters = distribution_clusters(Y_LOW, Y_HIGH, parts, &mut rng);

    let step = n.div_ceil(parts);

    for i in 0..n {
        pairs.push(HaversineDataPoint {
            x0: x_clusters[i / step].sample(&mut rng),
            y0: y_clusters[i / step].sample(&mut rng),
            x1: x_clusters[i / step].sample(&mut rng),
            y1: y_clusters[i / step].sample(&mut rng),
        });
    }
    HaversineData { pairs }
}

fn save_to_file(data: &HaversineData) {
    let file = File::create(format!("data_{}_flex.json", data.pairs.len()))
        .expect("Unable to create file");
    let writer = BufWriter::new(file);
    let mut serializer =
        serde_json::Serializer::with_formatter(writer, PrettyFormatter::with_indent(b"  "));
    data.serialize(&mut serializer)
        .expect("Unable to write data");
}

fn save_haversine_answer_to_file(data: &HaversineData) -> f64 {
    let pair_count = data.pairs.len();
    let file =
        File::create(format!("data_{pair_count}_haveranswer.f64")).expect("Unable to create file");
    let mut writer = BufWriter::new(file);

    let mut sum = 0f64;
    for point in &data.pairs {
        let dist = reference_haversine(point, EARTH_RADIUS);
        sum += dist;
        writer
            .write_f64::<LittleEndian>(dist)
            .expect("Failed to write to file");
    }

    #[allow(clippy::cast_precision_loss)]
    let avg = sum / pair_count as f64;
    writer
        .write_f64::<LittleEndian>(avg)
        .expect("Failed to write to file");

    writer.flush().expect("Failed to flush buffer");
    avg
}

fn main() {
    let args = Arguments::parse();
    let data = match args.dist {
        HaversineDist::Uniform => generate_haversine_data_uniform(args.pair_count, args.seed),
        HaversineDist::Cluster => generate_haversine_data_cluster(args.pair_count, args.seed),
    };
    save_to_file(&data);
    let avg = save_haversine_answer_to_file(&data);
    println!("Method: {}", args.dist);
    println!("Random seed: {}", args.seed);
    println!("Pair count: {}", args.pair_count);
    println!("Average: {avg:.16}");
}
