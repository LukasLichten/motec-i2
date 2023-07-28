use motec_i2::{I2Result, LDReader, beacon_builder};
use std::env;
use std::fs::File;

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

    let index = 0;
    let channel = &channels[index];
    println!(
        "Reading channel {}: {} ({} samples at {} Hz)",
        index, channel.name, channel.data_count, channel.sample_rate
    );
    println!("Channel: {:#?}", channel);

    let data = reader.channel_data(channel)?;
    for i in 0..6 {
        let sample = &data[i];
        let value = sample.decode_f64(channel);
        println!("[{}]: {:.1} - (Raw Sample: {:?})", i, value, sample);
    }

    // Demonstration of reading Lap Data from the Beacon using the Beacon builder
    let beacon = beacon_builder::get_beacon_channel(&channels).unwrap();
    let beacon_data = reader.channel_data(beacon)?;

    println!("Channel: {:#?}", beacon);

    if let (Some(event), Some(vehicle), Some(venue)) = (event, vehicle, venue) {
        println!("During the {} of {} at {} with {} on the {} following laps were set:",
                header.short_comment, // should be event.session, but for some reason the sample has there "2"
                event.name,
                venue.name,
                vehicle.id,
                header.date_string);
    }

    let laps = beacon_builder::get_laps_from_beacon(&beacon, &beacon_data);
    let mut index = 0;
    println!("{:#?}", laps);
    for l in &laps {
        // Insert parsing with your favorite timespan libaray
        // Except I could not find one
        // I spend significantly longer on looking for one, then writing and validating this mess
        let lap_time = l.lap_time;
        let (time_min, time_sec, time_ms) = (lap_time / 1000 / 60, (lap_time/1000)%60, lap_time%1000);
        let time = format!("{:0>2}:{:0>2}.{:0>3}", time_min, time_sec, time_ms); 

        if index != 0 && index != laps.len() - 1 { // Only flying laps
            println!("Lap {}: {}", index, time);
        }
        index += 1;
    }

    Ok(())
}
