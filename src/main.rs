use std::env;
use std::fs;

mod tinymod {
    use std::cmp;

    pub struct Sample {
        name: String,
        length: usize,
        finetune: i8,
        volume: u8,
        loop_start: usize,
        loop_len: usize,
    }

    impl Sample {
        pub fn load(sample_data: &[u8]) -> Sample {
            let mut name_vec = Vec::new();
            name_vec.extend_from_slice(&sample_data[0..22]);
            let name = String::from_utf8_lossy(&name_vec).to_string();
            let length_slice = &sample_data[22..24];
            let length = u16::from_be_bytes(length_slice.try_into().unwrap()) as usize;
            let mut finetune = sample_data[24] as i8;
            finetune &= 0x0F;
            if finetune >= 8 {
                finetune -= 16;
            }
            let volume = sample_data[25];

            let loop_start_slice = &sample_data[25..27];
            let loop_start = u16::from_be_bytes(loop_start_slice.try_into().unwrap()) as usize;
            let loop_len_slice = &sample_data[25..27];
            let loop_len = u16::from_be_bytes(loop_len_slice.try_into().unwrap()) as usize;
            Sample {
                name,
                length,
                finetune,
                volume,
                loop_start,
                loop_len,
            }
        }
    }

    #[derive(Clone, Copy)]
    struct Event {
        sample: usize,
        note: usize,
        fx: usize,
        fx_param: usize,
    }

    struct Row {
        events: Vec<Event>,
    }

    struct Pattern {
        rows: Vec<Row>,
    }

    impl Pattern {
        pub fn load(pattern_data: &[u8]) -> Pattern {
            let mut pattern = Pattern { rows: Vec::new() };

            for row_index in 0..64 {
                let mut row = Row { events: Vec::new() };
                for channel_index in 0..CHANNEL_COUNT {
                    let offset = channel_index * 4 + (row_index * CHANNEL_COUNT * 4);
                    let sample =
                        (pattern_data[offset] & 0xF0 | pattern_data[offset + 2] >> 4) as usize;
                    let fx = (pattern_data[offset + 2] & 0x0F) as usize;
                    let fx_param = (pattern_data[offset + 3]) as usize;
                    let mut note = 0;

                    let period = ((((pattern_data[offset] & 0x0F) as i16) << 8)
                        | pattern_data[offset + 1] as i16)
                        as isize;
                    let mut bestd = (period - BASE_P_TABLE[0]).abs();
                    if period > 0 {
                        for index in 1..61 {
                            let d = (period - BASE_P_TABLE[index]).abs();
                            if d < bestd {
                                bestd = d;
                                note = index;
                            }
                        }
                    }
                    println!("NOTE: {}", note);

                    row.events.push(Event {
                        sample,
                        fx,
                        fx_param,

                        note,
                    })
                }
                pattern.rows.push(row);
            }

            pattern
        }
    }

    const CHANNEL_COUNT: usize = 4;
    const BASE_P_TABLE: [isize; 61] = [
        0, 1712, 1616, 1525, 1440, 1357, 1281, 1209, 1141, 1077, 1017, 961, 907, 856, 808, 762,
        720, 678, 640, 604, 570, 538, 508, 480, 453, 428, 404, 381, 360, 339, 320, 302, 285, 269,
        254, 240, 226, 214, 202, 190, 180, 170, 160, 151, 143, 135, 127, 120, 113, 107, 101, 95,
        90, 85, 80, 76, 71, 67, 64, 60, 57,
    ];

    pub struct ModPlayer {
        pub name: String,
        pub samples: Vec<Sample>,
        patterns: Vec<Pattern>,
        // pattern_list: Vec<i8>,
        pattern_count: usize,
        // sample_count: isize,
        position_count: usize,
    }

    impl ModPlayer {
        pub fn load(module: Vec<u8>) -> ModPlayer {
            let mut large = false;

            let mut name_vec = Vec::new();
            name_vec.extend_from_slice(&module[0..20]);
            let name = String::from_utf8_lossy(&name_vec);

            let mut tag_vec = Vec::new();
            tag_vec.extend_from_slice(&module[1080..1084]);
            let tag = String::from_utf8_lossy(&tag_vec).to_string();
            println!("Tag: {}", tag);
            if tag == String::from("M.K.")
                || tag == String::from("M!K!")
                || tag == String::from("4TLF")
            {
                large = true;
            }

            let mut offset = 20;

            let mut sample_count = 15;
            if large {
                sample_count = 31;
            };
            let mut samples: Vec<Sample> = Vec::new();
            for _sample_index in 0..sample_count {
                samples.push(Sample::load(&module[offset..(offset + 81)]));
                offset += 30;
            }

            let position_count = module[offset] as usize;

            offset += 2;

            let mut pattern_count: usize = 1;

            for pos_index in 0..128 {
                pattern_count = cmp::max(pattern_count, module[offset + pos_index] as usize + 1);
                println!(
                    "PATTERNS {} - {}",
                    module[offset + pos_index] as usize,
                    pattern_count
                );
            }

            offset += 128;

            if large {
                offset += 4
            }

            let mut patterns: Vec<Pattern> = Vec::new();

            for pattern_index in 0..pattern_count {
                let data_start = offset + pattern_index * 4 * CHANNEL_COUNT * 64;
                let data_end = data_start + 4 * CHANNEL_COUNT * 64;
                println!("loading pattern {}", pattern_index);
                patterns.push(Pattern::load(&module[data_start..data_end]));
            }

            ModPlayer {
                name: name.to_string(),
                patterns,
                samples,
                position_count,
                pattern_count,
            }
        }
        pub fn play(self: Self) {}
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 {
        let module = fs::read(&args[1]).unwrap();
        let player = tinymod::ModPlayer::load(module);
        println!("PLAYING: {}", player.name);
        println!("Samples: {}", player.samples.len());
        player.play();
    } else {
        panic!("No module given");
    }
}
