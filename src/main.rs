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
        .build_cartesian_2d(0+5000..data.len(), 0.0f32..1.0)?;

    chart.configure_mesh().draw()?;

    chart
        //.draw_series(LineSeries::new(
        //    data.iter().enumerate().map(|(x, y)| (x, *y)),
        //    &RED,
        //))?
        .draw_series(
            data.iter().enumerate().map(|(x, y)| (x, *y)).map (|(x,y)| Circle::new((x,y),1, BLUE.filled())),
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
        .map(|x| ((*x ) * 1e4) as i32)
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
        .map(|x| ((*x ) * 64.0) as i8)
        .flat_map(|x| x.to_le_bytes())
        .map(|x| (x, 0))
        .fold(Vec::with_capacity(buffer.len()*2), |mut acc, p| {
            acc.push(p.0);
            acc.push(p.1);
            acc
        });

    println!("i lenght {}", i.len());
    f.write_all(&i)?;
    Ok(())
}



fn main() -> Result<(), Box<dyn Error>> {
    // Create "image"
    //let mut image: Vec<Vec<i32>> = Vec::new();

    //for i in 0..700 {
    //    let mut inner = Vec::new();
    //    for j in 0..1000 {
    //        inner.push((j / 10 + i / 5) % 5 * 63);
    //    }
    //    image.push(inner);
    //}

    //let image = image::open("test_pattern.png").unwrap();
    let image = image::open("omegav_inverted.jpg").unwrap();

    let image = image.resize_exact(PIXELS_PER_LINE as u32, 625, image::imageops::FilterType::Nearest).grayscale().into_luma8();
    image.save("processed_img.png").unwrap();

    image.view(0,312, PIXELS_PER_LINE as u32, 1).to_image().save("subimage_view.png").unwrap();

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

    let mut buffer = Vec::new();

    push_broad_sync_section(&mut buffer);
    push_short_sync_section(&mut buffer);
    // Line 6 to 310
    for i in 0..=304 {
        let img = image.view(0, i*2, PIXELS_PER_LINE as u32, 1).to_image();
        //let img = img.inner();
        if i*2 == 312 {
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
        let img = image.view(0, i*2+1, PIXELS_PER_LINE as u32, 1).to_image();
        push_image_scanline(&mut buffer, &img);
    }
    push_synced_halfline(&mut buffer);
    push_short_sync_section(&mut buffer);

    plot_samples(&buffer)?;

    //println!("{:?}", buffer);
    println!("Sample frequency {}", SAMPLE_FREQUENCY);
    println!("Pixels in whole scanline {}", SCANLINE_PIXELS);

    dump_as_sdriq(&buffer, "data.sdriq")?;
    dump_as_hackrf(&buffer, "data.bin")?;

    //dbg!(&image[1]);
    //
    //let mut f = std::fs::File::create("data.bin")?;

    //dbg!(&i[0..1000]);

    Ok(())
}
