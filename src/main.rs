fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut path = std::path::PathBuf::from(args.get(1).expect("Usage: apngquant image"));
    let decoder =
        png::Decoder::new(std::fs::File::open(path.as_path()).expect("Could not open file"));
    let reader = decoder.read_info()?;
    let mut reader = ReaderFrameIter::new(reader);
    let info = reader.info();
    let mut images: Vec<(Vec<u8>, [u32; 4])> = Vec::with_capacity(
        info.animation_control()
            .expect("Give APNG not PNG")
            .num_frames as usize,
    );
    if info.bit_depth != png::BitDepth::Eight || info.color_type != png::ColorType::Rgba {
        panic!("PNG not 32bpp RGBA; Bailing out because I'm too lazy to convert formats")
    }
    let liq = imagequant::new();
    let mut histo = liq.new_histogram();
    for (frame, frame_info, info) in &mut reader {
        let info = info.unwrap();
        images.push((
            frame.clone(),
            [info.x_offset, info.y_offset, info.width, info.height],
        ));
        let frame: Vec<imagequant::RGBA> = frame
            .chunks_exact(4)
            .map(|rgba| {
                imagequant::RGBA::new(
                    *rgba.get(0).unwrap(),
                    *rgba.get(1).unwrap(),
                    *rgba.get(2).unwrap(),
                    *rgba.get(3).unwrap(),
                )
            })
            .collect();
        let mut image = liq.new_image(
            &frame,
            frame_info.width as usize,
            frame_info.height as usize,
            0_f64,
        )?;
        histo.add_image(&mut image);
    }
    let mut res = histo.quantize()?;
    res.set_dithering_level(1.0);
    let mut stem = path.file_stem().unwrap().to_os_string();
    path.pop();
    stem.push("-fs8.png");
    path.push(stem);
    let file = std::fs::File::create(path)?;
    let file = std::io::BufWriter::new(file);
    let info = reader.info();
    let mut encoder = png::Encoder::new(file, info.width, info.height);
    encoder.set_animated(
        info.animation_control()
            .expect("Give APNG not PNG")
            .num_frames,
        0,
    )?;
    encoder.set_compression(png::Compression::Best);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_frame_delay(
        info.frame_control().unwrap().delay_num,
        info.frame_control().unwrap().delay_den,
    )?;
    encoder.set_palette(
        res.palette_ref()
            .iter()
            .flat_map(|color| color.rgb().iter().collect::<Vec<_>>())
            .collect::<Vec<_>>(),
    );
    encoder.set_trns(
        res.palette_ref()
            .iter()
            .map(|color| color.a)
            .collect::<Vec<_>>(),
    );
    encoder.set_dispose_op(png::DisposeOp::Background)?;
    let mut encoder = encoder.write_header()?;
    for (frame, dim) in images {
        let frame: Vec<imagequant::RGBA> = frame
            .chunks_exact(4)
            .map(|rgba| {
                imagequant::RGBA::new(
                    *rgba.get(0).unwrap(),
                    *rgba.get(1).unwrap(),
                    *rgba.get(2).unwrap(),
                    *rgba.get(3).unwrap(),
                )
            })
            .collect();
        let mut image = liq.new_image(&frame, dim[2] as usize, dim[3] as usize, 0_f64)?;
        let frame = res.remapped(&mut image)?.1;
        encoder.set_frame_position(0, 0)?;
        encoder.set_frame_dimension(dim[2], dim[3])?;
        encoder.set_frame_position(dim[0], dim[1])?;
        encoder.write_image_data(&frame)?;
    }
    Ok(())
}

struct ReaderFrameIter<T: std::io::Read>(png::Reader<T>);
impl<T: std::io::Read> Iterator for ReaderFrameIter<T> {
    type Item = (Vec<u8>, png::OutputInfo, Option<png::FrameControl>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf: Vec<u8> = vec![0; self.0.output_buffer_size()];
        let result = self.0.next_frame(&mut buf).ok()?;
        Some((buf, result, self.0.info().frame_control().copied()))
    }
}

impl<T: std::io::Read> ReaderFrameIter<T> {
    fn new(reader: png::Reader<T>) -> Self {
        Self(reader)
    }
    fn info(&self) -> &png::Info {
        self.0.info()
    }
}
