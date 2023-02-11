use futuresdr::anyhow::Result;
use futuresdr::blocks::Head;
use futuresdr::blocks::FileSink;
use futuresdr::blocks::Sink;
use futuresdr::blocks::VectorSink;
use futuresdr::blocks::NullSource;
use futuresdr::macros::connect;
use futuresdr::runtime::Flowgraph;
use futuresdr::runtime::Runtime;

fn main() -> Result<()> {
    let mut fg = Flowgraph::new();

    let src = NullSource::<u8>::new();
    let head = Head::<u8>::new(123);
//    let snk = FileSink::<u8>::new("test_output.bin");
    let snk = VectorSink::<u8>::new(123);
    let snk2 = Sink::new(|x: &u8| println!{"{}", x});

    dbg!(&snk);
    connect!(fg, src > head > snk2 > snk);

    Runtime::new().run(fg)?;


    Ok(())
}
