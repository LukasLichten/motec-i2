use motec_i2::{I2Result, LDReader, LDWriter};
use std::env;
use std::fs::File;

// Testing by reading the sample and writing back to test_write.ld
fn main() -> I2Result<()> {
    let path = env::args()
        .skip(1)
        .next()
        .unwrap_or("./samples/Sample1.ld".into());
    println!("Reading file: {}", path);

    let mut file = File::open(path).expect("Failed to open file!");
    let mut reader = LDReader::new(&mut file);

    let header = reader.read_header()?;
    println!("Header: {:#?}", header);

    let event = reader.read_event()?;
    println!("Event: {:#?}", event);

    let venue = reader.read_venue()?;
    println!("Venue: {:#?}", venue);

    let vehicle = reader.read_vehicle()?;
    println!("Vehicle: {:#?}", vehicle);

    let channels = reader.read_channels()?;
    println!("File has {} channels", channels.len());

    // let mut index = 0;
    // for channel in &channels {

    
    // println!(
    //     "Reading channel {}: {} ({} samples at {} Hz)",
    //     index, channel.name, channel.data_count, channel.sample_rate
    // );
    
    // index += 1;


    
    // }

    // let channel = &channels[69];

    // println!("Channel: {:#?}", channel);

    // let data = reader.channel_data(channel)?;
    // for i in 0..6 {
    //     let sample = &data[i*20];
    //     let value = sample.decode_f64(channel);
    //     println!("[{}]: {:.1} - (Raw Sample: {:?})", i*20, value, sample);
    // }

    let filename = "test_write.ld";
    let mut file = File::create(filename).expect("Failed to open file!");

    let mut writer = LDWriter::new(&mut file, header);
    
    for channel in &channels {
        writer = writer.with_channel(channel.clone(), reader.channel_data(channel)?);
    }
    
    writer.write()?;

    Ok(())
}
