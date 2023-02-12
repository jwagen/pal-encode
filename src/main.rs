use plotters::prelude::*;

use std::error::Error;

use std::io::Write;

fn plot_samples(data: &[f32]) -> Result<(), Box<dyn Error>> {
    let root = BitMapBackend::new("outplot.png", (1024, 768)).into_drawing_area();

    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Samples", ("sans-serif", 50).into_font())
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0..data.len(), 0.0f32..1.0)?;

    chart.configure_mesh().draw()?;

    chart
        .draw_series(LineSeries::new(
            data.iter().enumerate().map(|(x, y)| (x, *y)),
            &RED,
        ))?
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

const PIXELS_PER_LINE: usize = 208;
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
        for _ in 0..(SCANLINE_PIXELS / 2 - HSYNC_BROAD_PULSE) {
            buffer.push(0.3);
        }
        for _ in 0..HSYNC_BROAD_PULSE {
            buffer.push(0.0);
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

fn push_image_scanline(buffer: &mut Vec<f32>, line: &[i32]) {
    for _ in 0..HSYNC_PULSE {
        buffer.push(0.0);
    }
    for _ in 0..BACK_PORCH {
        buffer.push(0.3);
    }
    for n in 0..PIXELS_PER_LINE as usize {
        buffer.push((line[n] as f32 * 0.7 / 255.0) + 0.3);
    }
    for _ in 0..FRONT_PORCH {
        buffer.push(0.3);
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    // Create "image"
    let mut image: Vec<Vec<i32>> = Vec::new();

    for i in 0..700 {
        let mut inner = Vec::new();
        for j in 0..1000 {
            inner.push((j / 10 + i / 10) % 5 * 63);
        }
        image.push(inner);
    }

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
    for i in 6..=310 {
        push_image_scanline(&mut buffer, &image[i]);
    }
    push_short_sync_section(&mut buffer);
    // Field 2
    push_broad_sync_section(&mut buffer);
    push_short_sync_section(&mut buffer);
    push_blank_halfline(&mut buffer);
    for i in 319..=622 {
        push_image_scanline(&mut buffer, &image[i]);
    }
    push_synced_halfline(&mut buffer);
    push_short_sync_section(&mut buffer);

    plot_samples(&buffer);

    //println!("{:?}", buffer);
    println!("{}", SAMPLE_FREQUENCY);
    println!("{}", SCANLINE_PIXELS);

    //dbg!(&image[1]);
    //
    let mut f = std::fs::File::create("data.bin")?;

    let v = vec![1, 2, 3, 4, 5];

    let mut header = Vec::<u8>::new();
    header.extend_from_slice(&4000000i32.to_le_bytes());

    let i: Vec<_> = buffer
        .iter()
        .flat_map(|x| (((*x -0.5) * 1e6) as i32).to_le_bytes())
        .map(|x| (x, 0))
        .fold(Vec::with_capacity(v.len() * 2), |mut acc, p| {
            acc.push(p.0);
            acc.push(p.1);
            acc
        });

    println!("i lenght {}", i.len());
    f.write_all(&i)?;

    //dbg!(i);

    Ok(())
}
