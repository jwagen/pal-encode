use futuresdr::blocks::Apply;
use futuresdr::macros::connect;
use futuresdr::runtime::Flowgraph;
use plotters::prelude::*;

use std::error::Error;

use std::io::Write;
use std::path::Path;

use image::GenericImageView;

fn plot_samples(data: &[f32]) -> Result<(), Box<dyn Error>> {
    let root = BitMapBackend::new("outplot.png", (1024, 768)).into_drawing_area();

    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Samples", ("sans-serif", 50).into_font())
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0 + 5000..data.len(), 0.0f32..1.0)?;

    chart.configure_mesh().draw()?;

    chart
        //.draw_series(LineSeries::new(
        //    data.iter().enumerate().map(|(x, y)| (x, *y)),
        //    &RED,
        //))?
        .draw_series(
            data.iter()
                .enumerate()
                .map(|(x, y)| (x, *y))
                .map(|(x, y)| Circle::new((x, y), 1, BLUE.filled())),
        )?
        .label("y = x^2")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));
    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    root.present()?;

    Ok(())
}

//const PIXELS_PER_LINE: usize = 208;
const PIXELS_PER_LINE: usize = 520;
//const PIXELS_PER_LINE: usize = 1443;
const ACTIVE_PERIOD: f32 = 51.95e-6;
const PIXEL_FREQUENCY: f32 = PIXELS_PER_LINE as f32 / ACTIVE_PERIOD;

const SCANLINE_PIXELS: usize = FRONT_PORCH + HSYNC_PULSE + PIXELS_PER_LINE + BACK_PORCH;

const SAMPLE_FREQUENCY: f32 = SCANLINE_PIXELS as f32 / 64.0e-6;

const FRONT_PORCH: usize = (1.65e-6 * PIXEL_FREQUENCY) as usize;
const HSYNC_PULSE: usize = (4.7e-6 * PIXEL_FREQUENCY) as usize;
const BACK_PORCH: usize = (5.7e-6 * PIXEL_FREQUENCY) as usize;

const HSYNC_SHORT_PULSE: usize = (2.35e-6 * PIXEL_FREQUENCY) as usize;
const HSYNC_BROAD_PULSE: usize = ((64e-6 / 2.0 - 4.7e-6) * PIXEL_FREQUENCY) as usize;

// Push 5 half lines with short sync section
fn push_short_sync_section(buffer: &mut Vec<f32>) {
    for _ in 0..5 {
        for _ in 0..HSYNC_SHORT_PULSE {
            buffer.push(0.0);
        }
        for _ in 0..(SCANLINE_PIXELS / 2 - HSYNC_SHORT_PULSE) {
            buffer.push(0.3);
        }
    }
}

// Push 5 half lines with broad sync section
fn push_broad_sync_section(buffer: &mut Vec<f32>) {
    for _ in 0..5 {
        for _ in 0..HSYNC_BROAD_PULSE {
            buffer.push(0.0);
        }
        for _ in 0..(SCANLINE_PIXELS / 2 - HSYNC_BROAD_PULSE) {
            buffer.push(0.3);
        }
    }
}

fn push_blank_halfline(buffer: &mut Vec<f32>) {
    for _ in 0..(SCANLINE_PIXELS / 2) {
        buffer.push(0.3);
    }
}

fn push_synced_halfline(buffer: &mut Vec<f32>) {
    for _ in 0..HSYNC_PULSE {
        buffer.push(0.0);
    }
    for _ in 0..BACK_PORCH {
        buffer.push(0.3);
    }
    for _ in 0..(SCANLINE_PIXELS / 2 - HSYNC_PULSE - BACK_PORCH) {
        buffer.push(0.3);
    }
}

fn push_image_scanline(buffer: &mut Vec<f32>, line: &[u8]) {
    for _ in 0..HSYNC_PULSE {
        buffer.push(0.0);
    }
    for _ in 0..BACK_PORCH {
        buffer.push(0.3);
    }
    for n in 0..PIXELS_PER_LINE as usize {
        buffer.push(((line[n] as f32 * 0.7) / 255.0) + 0.3);
    }
    for _ in 0..FRONT_PORCH {
        buffer.push(0.3);
    }
}

fn dump_as_sdriq(buffer: &Vec<f32>, filename: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let mut f = std::fs::File::create(filename)?;

    let mut header = Vec::<u8>::new();
    // Sample rate S/s
    header.extend_from_slice(&4_000_000u32.to_le_bytes());
    // Center frequency in Hz
    header.extend_from_slice(&400_000_000u64.to_le_bytes());
    // Unix epoc timpestamp
    header.extend_from_slice(&0u64.to_le_bytes());
    // Sample size
    header.extend_from_slice(&24u32.to_le_bytes());
    // Zeros
    header.extend_from_slice(&0u32.to_le_bytes());
    // crc32 of prevoius bytes
    header.extend_from_slice(&crc32fast::hash(&header).to_le_bytes());

    let i: Vec<_> = buffer
        .iter()
        .map(|x| ((*x) * 1e4) as i32)
        .flat_map(|x| x.to_le_bytes())
        .map(|x| (x, 0))
        .fold(header, |mut acc, p| {
            acc.push(p.0);
            acc.push(p.1);
            acc
        });

    println!("i lenght {}", i.len());
    f.write_all(&i)?;
    Ok(())
}

fn dump_as_hackrf(buffer: &Vec<f32>, filename: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let mut f = std::fs::File::create(filename)?;

    let i: Vec<_> = buffer
        .iter()
        .map(|x| ((*x) * 64.0) as i8)
        .flat_map(|x| x.to_le_bytes())
        .map(|x| (x, 0))
        .fold(Vec::with_capacity(buffer.len() * 2), |mut acc, p| {
            acc.push(p.0);
            acc.push(p.1);
            acc
        });

    println!("i lenght {}", i.len());
    f.write_all(&i)?;
    Ok(())
}

fn modulate_frame(filename: impl AsRef<Path>) -> Result<Vec<f32>, Box<dyn Error>> {
    //let image = image::open("test_pattern.png").unwrap();
    let mut buffer = Vec::new();

    let image = image::open(filename).unwrap();

    let image = image
        .resize_exact(
            PIXELS_PER_LINE as u32,
            625,
            image::imageops::FilterType::Nearest,
        )
        .grayscale()
        .into_luma8();
    image.save("processed_img.png").unwrap();

    image
        .view(0, 312, PIXELS_PER_LINE as u32, 1)
        .to_image()
        .save("subimage_view.png")
        .unwrap();

    // Total lines 625

    // Choose pixel / line
    //For each line
    // whole line 64 us
    // Convert line to luma f32 0->1
    // Add horizontal sync
    //  - front porch push back - 1.65us
    //  - H sync pulse push front - 4.7us
    //  - back porch push front 5.7us
    //
    //

    push_broad_sync_section(&mut buffer);
    push_short_sync_section(&mut buffer);
    // Line 6 to 310
    for i in 0..=304 {
        let img = image.view(0, i * 2, PIXELS_PER_LINE as u32, 1).to_image();
        //let img = img.inner();
        if i * 2 == 312 {
            //dbg!(&img);
            img.save("312.png").unwrap();
        }
        push_image_scanline(&mut buffer, &img);
    }
    push_short_sync_section(&mut buffer);
    // Field 2
    push_broad_sync_section(&mut buffer);
    push_short_sync_section(&mut buffer);
    push_blank_halfline(&mut buffer);
    // line 319 to 622
    for i in 0..=303 {
        let img = image
            .view(0, i * 2 + 1, PIXELS_PER_LINE as u32, 1)
            .to_image();
        push_image_scanline(&mut buffer, &img);
    }
    push_synced_halfline(&mut buffer);
    push_short_sync_section(&mut buffer);

    //    plot_samples(&buffer)?;

    println!("Sample frequency {}", SAMPLE_FREQUENCY);
    println!("Pixels in whole scanline {}", SCANLINE_PIXELS);

    Ok(buffer)
}

use futuresdr::anyhow::Result;
use futuresdr::async_trait::async_trait;
use futuresdr::blocks::zeromq::PubSinkBuilder;
use futuresdr::blocks::FileSink;
use futuresdr::blocks::NullSink;
use futuresdr::macros::message_handler;
use futuresdr::num_complex::Complex32;
use futuresdr::runtime::config;
use futuresdr::runtime::Block;
use futuresdr::runtime::BlockMeta;
use futuresdr::runtime::BlockMetaBuilder;
use futuresdr::runtime::Kernel;
use futuresdr::runtime::MessageIo;
use futuresdr::runtime::MessageIoBuilder;
use futuresdr::runtime::Pmt;
use futuresdr::runtime::Runtime;
use futuresdr::runtime::StreamIo;
use futuresdr::runtime::StreamIoBuilder;
use futuresdr::runtime::WorkIo;
use futuresdr::blocks::seify;
struct LumaModulator {
    filename: String,
    frame_counter: u32,
}

impl LumaModulator {
    pub fn new() -> Block {
        Block::new(
            BlockMetaBuilder::new("Luma Mudulator").build(),
            StreamIoBuilder::new().add_output::<f32>("out").build(),
            MessageIoBuilder::new()
                .add_input("ctrl_port", Self::change_filename)
                .build(),
            Self {
                filename: "images/test_pattern.png".to_string(),
                frame_counter: 0,
            },
        )
    }

    #[message_handler]
    async fn change_filename(
        &mut self,
        _io: &mut WorkIo,
        _mio: &mut MessageIo<Self>,
        _meta: &mut BlockMeta,
        p: Pmt,
    ) -> Result<Pmt> {
        println!("Got message {:?}", p);
        if let Pmt::String(s) = p {
            println!("Changed filename to {}", s);
            self.filename = s;
        }
        Ok(Pmt::Null)
    }
}

#[async_trait]
impl Kernel for LumaModulator {
    async fn work(
        &mut self,
        io: &mut WorkIo,
        sio: &mut StreamIo,
        _mio: &mut MessageIo<Self>,
        _meta: &mut BlockMeta,
    ) -> Result<()> {
        let mut buffer = sio.output(0).slice::<f32>();

        if buffer.len() >= (SAMPLE_FREQUENCY/25.0) as usize {
            println!("Starting modulate_frame with filename: {}", self.filename);
            let res = modulate_frame(&self.filename).unwrap();
            //dbg!(&res[10000..11000]);

            buffer[0..res.len()].clone_from_slice(&res);
            sio.output(0).produce(res.len());
        }
        else {
            println!("Buffer full");
            sio.output(0).produce(0);
        }
        self.frame_counter += 1;
        if self.frame_counter == 1000 {
            io.finished = true;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let c = config::config();
    let luma = LumaModulator::new();

    let mut fg = Flowgraph::new();

    //let luma = fg.add_block(luma);
    //let file_sink = FileSink::<f32>::new("futuresdr_dump.cf32");
    let snk = NullSink::<f32>::new();
    let file_sink = FileSink::<f32>::new("futuresdr_dump.f32");
    let gnc_sink = PubSinkBuilder::<f32>::new()
        .address("tcp://127.0.0.1:1338")
        .build();


    let hackrf_sink = seify::SinkBuilder::new()
    //.args("driver=hackrf")?
    .frequency(182.25e6)
    .sample_rate(10e6)
    .gain(-40.0)
    .build().expect("Unable to open sdr hardware");

    let float_to_complex = Apply::new(|s: &f32| Complex32::new(*s, 0.0));

    let invert = Apply::new(|s: &f32| 1.0-*s);

    //fg.connect_stream(luma, "out", file_sink, "in");
    connect!(fg, 
        //luma > snk; 
        //luma > file_sink;
        luma > invert > float_to_complex > hackrf_sink;
    );

    Runtime::new().run(fg)?;

    Ok(())
}
