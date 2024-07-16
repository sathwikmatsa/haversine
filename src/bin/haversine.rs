use std::{collections::VecDeque, fs::File, io::BufReader, path::PathBuf};

use byteorder::{LittleEndian, ReadBytesExt};
use clap::{error::ErrorKind, CommandFactory, Parser};
use haversine::{reference_haversine, HaversineData, EARTH_RADIUS};
use memmap2::MmapOptions;

#[derive(Parser, Debug)]
struct Arguments {
    #[arg(name = "haversine_input.json")]
    data_file: PathBuf,
    #[arg(name = "answers.f64")]
    answer_file: Option<PathBuf>,
}

fn pop_next_answer(answers: &mut VecDeque<f64>) -> f64 {
    if let Some(ans) = answers.pop_front() {
        ans
    } else {
        eprintln!("Error: validation input exhausted");
        std::process::exit(1);
    }
}

struct InputConf {
    input: HaversineData,
    input_size: usize,
    answers: VecDeque<f64>,
    validate: bool,
}

#[perf::instrument]
fn read_input(input_json: File, validation_answers_f64: Option<File>) -> InputConf {
    let mmap = unsafe {
        MmapOptions::new()
            .map(&input_json)
            .expect("create file mmap")
    };
    drop(input_json);
    let input_size = mmap.len();
    // let input: HaversineData = serde_json::from_slice(&mmap).expect("deserialize input data");
    let input = HaversineData::parse_from_json_slice(&mmap).expect("deserialize input data");
    let validate = validation_answers_f64.is_some();

    let answers: VecDeque<f64> = match validation_answers_f64 {
        None => VecDeque::new(),
        #[allow(clippy::uninit_vec)]
        Some(f) => {
            let ans_mmap = unsafe {
                MmapOptions::new()
                    .map(&f)
                    .expect("create answers file mmap")
            };
            let mut buf_reader = BufReader::new(f);
            let f64_array_size = ans_mmap.len() / std::mem::size_of::<f64>();
            let mut buffer: Vec<f64> = Vec::with_capacity(f64_array_size);
            unsafe { buffer.set_len(f64_array_size) }
            buf_reader
                .read_f64_into::<LittleEndian>(&mut buffer[..])
                .expect("read f64s");
            buffer.into()
        }
    };

    InputConf {
        input,
        input_size,
        answers,
        validate,
    }
}

fn calculate_haversine_with_validation(input_json: File, validation_answers_f64: Option<File>) {
    let InputConf {
        input,
        input_size,
        mut answers,
        validate,
    } = read_input(input_json, validation_answers_f64);

    let mut sum = 0f64;
    let pair_count = input.pairs.len();

    for point in input.pairs {
        let dist = reference_haversine(&point, EARTH_RADIUS);
        sum += dist;
        if validate {
            let ans = pop_next_answer(&mut answers);
            // Note(sathwik): The error margin is configured after trail and error.
            // Need to dig into serde's f64 serialize precision for a better understanding.
            if (dist - ans).abs() > 1e-10 {
                eprintln!(
                    "Failed validation for {:?}. Got {} Expected {} Diff {}",
                    point,
                    dist,
                    ans,
                    (dist - ans).abs()
                );
                std::process::exit(1);
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let avg = sum / pair_count as f64;
    println!("Input size: {input_size}");
    println!("Pair count: {pair_count}");
    println!("Haversine avg: {avg}");

    if validate {
        let ref_avg = pop_next_answer(&mut answers);
        println!();
        println!("Validation:");
        println!("Reference avg: {ref_avg}");
        println!("Difference: {}", ref_avg - avg);
    }
    println!();
}

fn main() {
    perf::begin_profile();
    let args = Arguments::parse();
    let input = match File::open(&args.data_file) {
        Ok(f) => f,
        Err(e) => Arguments::command()
            .error(
                ErrorKind::Io,
                format!("Unable to open `{}`: {}", args.data_file.display(), e),
            )
            .exit(),
    };

    let answers = match args.answer_file {
        Some(p) => match File::open(&p) {
            Ok(f) => Some(f),
            Err(e) => Arguments::command()
                .error(
                    ErrorKind::Io,
                    format!("Unable to open `{}`: {}", p.display(), e),
                )
                .exit(),
        },
        None => None,
    };
    calculate_haversine_with_validation(input, answers);
    perf::end_and_print_profile();
}
