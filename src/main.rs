extern crate sdl2;
use sdl2::audio::{AudioCallback, AudioSpecDesired};
use std::env;
use std::fs;
use std::io::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tinymod::ModPlayer;

mod tinymod {
    use std::cmp;

    const PAULARATE: usize = 3740000; // approx. pal timing
    const OUTRATE: usize = 48000; // approx. pal timing
    const OUTFPS: usize = 50; // approx. pal timing

    struct Voice {
        pos: f32,
        pub sample: Option<usize>,
        pub period: isize,
        pub volume: isize,
        sample_length: usize,
        loop_length: usize,
    }

    impl Voice {
        fn new() -> Voice {
            Voice {
                pos: 0.0,
                period: 65535,
                volume: 0,
                sample: None,
                sample_length: 0,
                loop_length: 1,
            }
        }

        fn render(&mut self, sample: &Sample, buffer: &mut [f32], samples: usize, offset: usize) {
            for i in 0..samples {
                self.pos += (PAULARATE as f32 / self.period as f32) / OUTRATE as f32;
                let mut int_pos = self.pos.floor() as usize;

                if int_pos >= self.sample_length {
                    self.pos -= self.loop_length as f32;
                    int_pos -= self.loop_length;
                }
                let mut next_pos = int_pos + 1;
                if next_pos >= self.sample_length {
                    next_pos -= self.loop_length
                }

                let next_fac = self.pos - self.pos.floor();
                let inv_fac = 1.0 - next_fac;

                let sample_value =
                    sample.data[int_pos] as f32 * inv_fac + sample.data[next_pos] as f32 * next_fac;
                buffer[i * 2 + offset] +=
                    (sample_value / 128.0 * (self.volume as f32 / 64.0)) * 0.5;
            }
        }

        fn trigger(
            &mut self,
            sample_index: usize,
            sample_length: usize,
            loop_length: usize,
            offset: isize,
        ) {
            println!(
                "Trig: {} {} {} {}",
                sample_index, sample_length, loop_length, offset
            );
            self.sample = Some(sample_index);
            self.sample_length = sample_length;
            self.loop_length = loop_length;
            self.pos = (offset as f32).min(sample_length as f32 - 1.0);
        }
    }

    fn clamp<T>(x: T, min: T, max: T) -> T
    where
        T: Ord,
    {
        cmp::max(min, cmp::min(max, x))
    }

    pub struct Sample {
        name: String,
        length: usize,
        finetune: i8,
        volume: u8,
        loop_start: usize,
        loop_len: usize,
        data: Vec<i8>,
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
            println!("VOL: {}", volume);

            let loop_start_slice = &sample_data[26..28];
            let loop_start = u16::from_be_bytes(loop_start_slice.try_into().unwrap()) as usize;
            let loop_len_slice = &sample_data[28..30];
            let loop_len = u16::from_be_bytes(loop_len_slice.try_into().unwrap()) as usize;
            let data: Vec<i8> = Vec::new();
            Sample {
                name,
                length,
                finetune,
                volume,
                loop_start,
                loop_len,
                data,
            }
        }
        pub fn load_data(&mut self, pcm: &[u8]) -> usize {
            println!("LOAD SAMPLE: {}", self.length * 2);
            if self.length == 0 {
                return 0;
            }
            for byte in 0..(self.length * 2) {
                self.data.push(pcm[byte] as i8);
            }
            self.length * 2
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

    struct Channel {
        note: usize,
        period: usize,
        sample: usize,
        fine_tune: isize,
        volume: usize,
        loop_start: usize,
        loop_count: usize,
        retrig_count: usize,
        vib_wave: usize,
        vib_retr: usize,
        vib_pos: usize,
        vib_ampl: usize,
        vib_speed: usize,
        trem_wave: usize,
        trem_retr: usize,
        trem_pos: usize,
        trem_ampl: usize,
        trem_speed: usize,
        fx_buf: [usize; 16],
        fx_buf14: [usize; 16],
    }

    impl Channel {
        pub fn new() -> Channel {
            Channel {
                note: 0,
                period: 0,
                sample: 0,
                fine_tune: 0,
                volume: 0,
                loop_start: 0,
                loop_count: 0,
                retrig_count: 0,
                vib_wave: 0,
                vib_retr: 0,
                vib_pos: 0,
                vib_ampl: 0,
                vib_speed: 0,
                trem_wave: 0,
                trem_retr: 0,
                trem_pos: 0,
                trem_ampl: 0,
                trem_speed: 0,
                fx_buf: [0; 16],
                fx_buf14: [0; 16],
            }
        }
        fn get_period(
            &mut self,
            p_table: &Vec<Vec<i32>>,
            mut offs: isize,
            fine_offs: isize,
        ) -> usize {
            let mut ft: isize = self.fine_tune + fine_offs;
            while ft > 7 {
                offs += 1;
                ft -= 16;
            }
            while ft < -8 {
                offs -= 1;
                ft += 16;
            }
            if self.note > 0 {
                let clamped =
                    clamp(self.note as isize + offs - 1, 0 as isize, 59 as isize) as usize;
                return p_table[ft as usize & 0x0f][clamped] as usize;
            }
            0
        }
        fn set_period(&mut self, p_table: &Vec<Vec<i32>>, offs: isize, fine_offs: isize) {
            if self.note > 0 {
                self.period = self.get_period(p_table, offs, fine_offs);
            }
        }
    }

    pub struct ModPlayer {
        pub name: String,
        pub samples: Vec<Sample>,
        patterns: Vec<Pattern>,
        pattern_list: Vec<usize>,
        pattern_count: usize,
        // sample_count: isize,
        position_count: usize,
        p_table: Vec<Vec<i32>>,
        vib_table: Vec<Vec<Vec<i32>>>,
        speed: usize,
        tick_rate: usize,
        tr_counter: usize,
        cur_tick: usize,
        cur_row: isize,
        cur_pos: usize,
        delay: usize,
        channels: Vec<Channel>,

        voices: Vec<Voice>,
    }

    impl ModPlayer {
        pub fn load(module: Vec<u8>) -> ModPlayer {
            // generate tables

            let mut p_table: Vec<Vec<i32>> = Vec::new();

            for ft in 0..16 {
                let rft: i32 = -(if ft > 8 { ft - 16 } else { ft });
                let fac: f32 = (2.0_f32).powf((rft as f32) / (12.0 * 16.0));
                let mut inner: Vec<i32> = Vec::new();
                for i in 0..60 {
                    let entry = ((BASE_P_TABLE[i] as f32) * fac) as i32;
                    inner.push(entry);
                }
                p_table.push(inner);
            }

            let mut vib_table = Vec::new();

            let mut vib_0 = Vec::new();
            let mut vib_1 = Vec::new();
            let mut vib_2 = Vec::new();

            for ampl in 0..15 {
                let mut vib_0_inner = Vec::new();
                let mut vib_1_inner = Vec::new();
                let mut vib_2_inner = Vec::new();
                let scale = (ampl as f32) + 1.5;
                for x in 0..64 {
                    let vib_0_entry = (scale * ((x as f32) / 32.0).sin()) as i32;
                    vib_0_inner.push(vib_0_entry);
                    let vib_1_entry = (scale * ((63 - x) as f32 / 31.5 - 1.0)) as i32;
                    vib_1_inner.push(vib_1_entry);
                    let vib_2_entry = (scale * (if x < 32 { 1.0 } else { -1.0 })) as i32;
                    vib_2_inner.push(vib_2_entry);
                }
                vib_0.push(vib_0_inner);
                vib_1.push(vib_1_inner);
                vib_2.push(vib_2_inner);
            }
            vib_table.push(vib_0);
            vib_table.push(vib_1);
            vib_table.push(vib_2);

            // Paula stuff

            let voices: [Voice; 4] = [Voice::new(), Voice::new(), Voice::new(), Voice::new()];

            // load module

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
            let mut pattern_list: Vec<usize> = Vec::new();
            for pos_index in 0..128 {
                pattern_count = cmp::max(pattern_count, module[offset + pos_index] as usize + 1);
                pattern_list.push(module[offset + pos_index] as usize);
            }

            offset += 128;

            if large {
                offset += 4
            }

            let mut patterns: Vec<Pattern> = Vec::new();

            for pattern_index in 0..pattern_count {
                let data_start = offset + pattern_index * 4 * CHANNEL_COUNT * 64;
                let data_end = data_start + 4 * CHANNEL_COUNT * 64;
                patterns.push(Pattern::load(&module[data_start..data_end]));
            }
            println!("OFFSET {} - {}", offset, pattern_count);

            offset = 1084 + ((pattern_count) * 1024);

            for sample in samples.iter_mut() {
                let length = sample.length;
                println!(
                    "load PCM l: {} ls: {} ll: {}",
                    length, sample.loop_start, sample.loop_len
                );
                offset += sample.load_data(&module[offset..(offset + length * 2)]);
            }

            let mut channels: Vec<Channel> = Vec::new();
            for _ch in 0..4 {
                channels.push(Channel::new())
            }

            let mut player = ModPlayer {
                name: name.to_string(),
                patterns,
                samples,
                position_count,
                pattern_count,
                pattern_list,
                p_table,
                vib_table,
                speed: 6,
                tick_rate: 0,
                tr_counter: 0,
                cur_tick: 0,
                cur_row: 0,
                cur_pos: 0,
                delay: 0,
                channels,
                voices: Vec::from(voices),
            };

            player.calc_tick_rate(125);
            player
        }

        fn calc_tick_rate(&mut self, bpm: usize) {
            self.tick_rate = 125 * OUTRATE / (bpm * OUTFPS);
        }

        fn trig_note(&mut self, channel_index: usize, event: &Event) {
            let mut offset: usize = 0;
            if event.fx == 9 {
                let channel = &mut self.channels[channel_index];
                offset = channel.fx_buf[9] << 8;
            }
            if event.fx != 3 && event.fx != 5 {
                let channel = &mut self.channels[channel_index];
                let sample = &self.samples[channel.sample - 1];
                channel.set_period(&self.p_table, 0, 0);

                let voice: &mut Voice = &mut self.voices[channel_index];
                if sample.loop_len > 2 {
                    voice.trigger(
                        channel.sample - 1,
                        2 * (sample.loop_start + sample.loop_len),
                        2 * sample.loop_len,
                        offset as isize,
                    )
                } else {
                    voice.trigger(channel.sample - 1, sample.length * 2, 2, offset as isize)
                }
                if channel.vib_retr > 0 {
                    channel.vib_pos = 0;
                }
                if channel.trem_retr > 0 {
                    channel.trem_pos = 0;
                }
            }
        }

        fn tick(&mut self) {
            for ch in 0..4 {
                let pattern = &self.patterns[self.pattern_list[self.cur_pos]];
                let row = &pattern.rows[self.cur_row as usize];
                let event = row.events[ch];

                let fxpl = event.fx_param & 0x0F;
                let mut trem_vol: usize = 0;
                if self.cur_tick == 0 {
                    if event.sample > 0 {
                        let channel = &mut self.channels[ch];
                        channel.sample = event.sample;
                        channel.fine_tune = self.samples[channel.sample].finetune as isize;
                        channel.volume = self.samples[channel.sample].volume as usize;
                    }
                    if event.fx_param > 0 {
                        let channel = &mut self.channels[ch];
                        channel.fx_buf[event.fx] = event.fx_param
                    }
                    if event.note > 0 && (event.fx != 14 || ((event.fx_param >> 4) != 13)) {
                        let channel = &mut self.channels[ch];
                        channel.note = event.note;
                        self.trig_note(ch, &event);
                    }

                    match event.fx {
                        4 | 6 => {
                            let channel = &mut self.channels[ch];
                            if channel.fx_buf[4] & 0x0f > 0 {
                                channel.vib_ampl = channel.fx_buf[4] & 0x0f;
                            }
                            if channel.fx_buf[4] & 0xf0 > 0 {
                                channel.vib_speed = channel.fx_buf[4] >> 4;
                            }
                            if channel.vib_ampl > 0 {
                                channel.set_period(
                                    &self.p_table,
                                    0,
                                    self.vib_table[channel.vib_wave][(channel.vib_ampl) - 1]
                                        [channel.vib_pos]
                                        as isize,
                                );
                            }
                        }
                        7 => {
                            let channel = &mut self.channels[ch];
                            if channel.fx_buf[7] & 0x0f > 0 {
                                channel.trem_ampl = channel.fx_buf[7] & 0x0f;
                            }
                            if channel.fx_buf[7] & 0xf0 > 0 {
                                channel.trem_speed = channel.fx_buf[7] >> 4;
                            }
                            trem_vol = self.vib_table[channel.trem_wave][(channel.trem_ampl) - 1]
                                [channel.trem_pos] as usize;
                        }
                        12 => {
                            let channel = &mut self.channels[ch];
                            channel.volume = clamp(event.fx_param, 0, 64);
                        }
                        14 => {
                            if fxpl > 0 {
                                let channel = &mut self.channels[ch];
                                channel.fx_buf14[event.fx_param >> 4] = fxpl;
                            }
                            match event.fx_param >> 4 {
                                0 => {}
                                1 => {
                                    let channel = &mut self.channels[ch];
                                    channel.period =
                                        cmp::max(113, channel.period - channel.fx_buf14[1]);
                                }
                                2 => {
                                    let channel = &mut self.channels[ch];
                                    channel.period =
                                        cmp::min(856, channel.period + channel.fx_buf14[1]);
                                }
                                3 => {}
                                4 => {
                                    let channel = &mut self.channels[ch];
                                    channel.vib_wave = fxpl & 3;
                                    if channel.vib_wave == 3 {
                                        channel.vib_wave = 0;
                                    }
                                    channel.vib_retr = fxpl & 4;
                                }
                                5 => {
                                    let channel = &mut self.channels[ch];
                                    channel.fine_tune = fxpl as isize;
                                    if channel.fine_tune >= 8 {
                                        channel.fine_tune -= 16
                                    }
                                }
                                7 => {
                                    let channel = &mut self.channels[ch];
                                    channel.trem_wave = fxpl & 3;
                                    if channel.trem_wave == 3 {
                                        channel.trem_wave = 0;
                                    }
                                    channel.trem_retr = fxpl & 4;
                                }
                                9 => {
                                    let channel = &self.channels[ch];
                                    if channel.fx_buf14[9] > 0 && event.note == 0 {
                                        self.trig_note(ch, &event);
                                        let channel = &mut self.channels[ch];
                                        channel.retrig_count = 0;
                                    }
                                }
                                10 => {
                                    let channel = &mut self.channels[ch];
                                    channel.volume =
                                        cmp::min(channel.volume + channel.fx_buf14[10], 64);
                                }
                                11 => {
                                    let channel = &mut self.channels[ch];
                                    channel.volume =
                                        cmp::max(channel.volume - channel.fx_buf14[11], 0);
                                }
                                14 => {
                                    let channel = &mut self.channels[ch];
                                    self.delay = channel.fx_buf14[14];
                                }
                                15 => {}
                                _ => {}
                            };
                        }
                        15 => {
                            if event.fx_param > 0 {
                                if event.fx_param <= 32 {
                                    self.speed = event.fx_param;
                                } else {
                                    self.calc_tick_rate(event.fx_param);
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    match event.fx {
                        0 => {
                            // arpeggio
                            if event.fx_param > 0 {
                                let mut no: usize = 0;
                                let channel = &mut self.channels[ch];
                                match self.cur_tick % 3 {
                                    1 => no = event.fx_param >> 4,
                                    2 => no = event.fx_param & 0x0F,
                                    _ => {}
                                }
                                channel.set_period(&self.p_table, no as isize, 0);
                            }
                        }
                        1 => {
                            // slide up
                            let channel = &mut self.channels[ch];
                            channel.period = cmp::max(113, channel.period - channel.fx_buf[1]);
                        }
                        2 => {
                            // slide down
                            let channel = &mut self.channels[ch];
                            channel.period = cmp::min(856, channel.period + channel.fx_buf[2]);
                        }
                        3 | 5 => {
                            let channel = &mut self.channels[ch];
                            // slide plus volslide
                            if event.fx == 5 {
                                if channel.fx_buf[5] & 0xf0 > 0 {
                                    channel.volume =
                                        cmp::min(channel.volume + (channel.fx_buf[5] >> 4), 64);
                                } else {
                                    channel.volume =
                                        cmp::max(channel.volume - (channel.fx_buf[5] & 0x0F), 0);
                                }
                            }
                            let np = channel.get_period(&self.p_table, 0, 0);
                            if channel.period > np {
                                channel.period = cmp::max(channel.period - channel.fx_buf[3], np);
                            } else {
                                channel.period = cmp::min(channel.period + channel.fx_buf[3], np);
                            }
                        }
                        4 | 6 => {
                            let channel = &mut self.channels[ch];
                            if event.fx == 6 {
                                if channel.fx_buf[6] & 0xf0 > 0 {
                                    channel.volume =
                                        cmp::min(channel.volume + (channel.fx_buf[6] >> 4), 64);
                                } else {
                                    channel.volume =
                                        cmp::max(channel.volume - (channel.fx_buf[6] & 0x0F), 0);
                                }
                            }
                            if channel.vib_ampl > 0 {
                                channel.set_period(
                                    &self.p_table,
                                    0,
                                    self.vib_table[channel.vib_wave][channel.vib_ampl - 1]
                                        [channel.vib_pos]
                                        as isize,
                                );
                            }
                            channel.vib_pos = (channel.vib_pos + channel.vib_speed) & 0x3F;
                        }
                        7 => {
                            let channel = &mut self.channels[ch];
                            trem_vol = self.vib_table[channel.trem_wave][channel.trem_ampl - 1]
                                [channel.trem_pos] as usize;
                            channel.trem_pos = (channel.trem_pos + channel.trem_speed) & 0x3F;
                        }
                        10 => {
                            let channel = &mut self.channels[ch];
                            if channel.fx_buf[10] & 0xF0 > 0 {
                                channel.volume =
                                    cmp::min(channel.volume + (channel.fx_buf[10] >> 4), 64);
                            } else {
                                println!(
                                    "VOLSLIDE DOWN {} - {}",
                                    channel.volume, channel.fx_buf[10]
                                );
                                channel.volume = cmp::max(
                                    channel.volume as isize - (channel.fx_buf[10] as isize & 0x0F),
                                    0,
                                ) as usize;
                            }
                        }
                        11 => {
                            if self.cur_tick == self.speed - 1 {
                                self.cur_row = -1;
                                self.cur_pos = event.fx_param;
                            }
                        }
                        13 => {
                            if self.cur_tick == self.speed - 1 {
                                self.cur_pos += 1;
                                self.cur_row = ((10 * (event.fx_param >> 4)
                                    + (event.fx_param & 0x0F))
                                    - 1) as isize;
                            }
                        }
                        14 => match event.fx_param >> 4 {
                            6 => {
                                let channel = &mut self.channels[ch];
                                if fxpl == 0 {
                                    channel.loop_start = self.cur_row as usize;
                                } else if self.cur_tick == self.speed - 1 {
                                    if channel.loop_count < fxpl {
                                        self.cur_row = (channel.loop_start - 1) as isize;
                                        channel.loop_count += 1;
                                    } else {
                                        channel.loop_count = 0;
                                    }
                                }
                            }
                            9 => {
                                let channel = &mut self.channels[ch];
                                channel.retrig_count += 1;
                                if channel.retrig_count == channel.fx_buf14[9] {
                                    channel.retrig_count = 0;
                                    self.trig_note(ch, &event);
                                }
                            }
                            12 => {
                                // cut
                                let channel = &mut self.channels[ch];
                                if self.cur_tick == channel.fx_buf14[12] {
                                    channel.volume = 0;
                                }
                            }
                            13 => {
                                // delay
                                let channel = &mut self.channels[ch];
                                if self.cur_tick == channel.fx_buf14[13] {
                                    self.trig_note(ch, &event)
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
                let voice = &mut self.voices[ch];
                let channel = &mut self.channels[ch];
                voice.volume = clamp(channel.volume + trem_vol, 0, 64) as isize;
                voice.period = channel.period as isize;
            }

            self.cur_tick += 1;
            if self.cur_tick >= self.speed * (self.delay + 1) {
                self.cur_tick = 0;
                self.cur_row += 1;
                self.delay = 0;
            }
            if self.cur_row >= 64 {
                self.cur_row = 0;
                self.cur_pos += 1;
                println!(
                    "NEXT_PATTERN POS:{} PTN: {}",
                    self.cur_pos, self.pattern_list[self.cur_pos]
                );
            }
            if self.cur_pos > self.position_count {
                self.cur_pos = 0;
            }
        }

        fn paula_render(&mut self, out_buf: &mut [f32], samples: usize, offset: usize) {
            for ch in 0..4 {
                let voice = &mut self.voices[ch];
                let sample_index = voice.sample;
                match sample_index {
                    None => {}
                    Some(index) => {
                        let sample = &self.samples[index];
                        if ch == 0 || ch == 3 {
                            voice.render(sample, out_buf, samples, offset);
                        } else {
                            voice.render(sample, out_buf, samples, offset + 1);
                        }
                    }
                }
            }
        }

        pub fn render(&mut self, buf: &mut [f32]) {
            //println!("R: {}, TR: {}", buf.len(), self.tick_rate);
            let mut len = buf.len() / 2;
            let mut out_pointer = 0;
            for i in 0..buf.len() {
                buf[i] = 0.0;
            }
            while len > 0 {
                let todo = cmp::min(len, self.tr_counter);
                if todo > 0 {
                    self.paula_render(buf, todo, out_pointer);
                    // out_pointer += todo;
                    out_pointer += 2 * todo;
                    len -= todo;
                    self.tr_counter -= todo;
                } else {
                    self.tick();
                    self.tr_counter = self.tick_rate;
                }
            }
        }
    }
}

struct SDLSound {
    player: ModPlayer,
}

impl AudioCallback for SDLSound {
    type Channel = f32;

    fn callback(&mut self, out: &mut [Self::Channel]) {
        self.player.render(out);
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 {
        let term = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term))?;
        let module = fs::read(&args[1]).unwrap();
        let player: tinymod::ModPlayer = tinymod::ModPlayer::load(module);
        println!("PLAYING: {}", player.name);
        println!("Samples: {}", player.samples.len());

        let sdl_context = sdl2::init().unwrap();
        let audio_subsystem = sdl_context.audio().expect("Audio system failed");

        let desired_spec = AudioSpecDesired {
            freq: Some(48_000),
            channels: Some(2), // mono
            samples: None,     // default sample size
        };
        let device = audio_subsystem
            .open_playback(None, &desired_spec, |spec| {
                println!("Spec: {:?}", spec);
                SDLSound { player }
            })
            .expect("Device open failed");
        device.resume();
        while !term.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(200));
        }
    } else {
        panic!("No module given");
    }
    Ok(())
}
