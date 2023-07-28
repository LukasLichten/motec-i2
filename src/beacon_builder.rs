use crate::{ChannelMetadata, ChannelFlag, Datatype, Sample};


// This file serves to offer functions to build a beacon channel from lapdata
// And also to convert a beacon channel to lapdata


#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Lap {
    /// laptime stored in ms
    pub lap_time: i32,
    /// Sector Splits stored in ms
    /// Number of sector splits can vary between laps
    /// Final sector time is not included in this vector, as it is a conclusion of the laptime minus sum of sectors
    pub sectors: Vec<i32>
}

/// This searches through all the channels and returns the Beacon channel (if present)
pub fn get_beacon_channel(channels: &Vec<ChannelMetadata>) -> Option<&ChannelMetadata> {
    for c in channels {
        match c.channel_feature_flag {
            ChannelFlag::Beacon => {
                match c.datatype {
                    Datatype::Beacon16 | Datatype::Beacon32 => return Some(c),
                    _ => ()
                }
            },
            _ => ()
        };
    }

    None
}

/// This returns the laps from a beacon channel
/// First lap is the outlap, last lap is the inlap
/// This will also include the times for the sectors, except for the last sector, which you may calculate out of the laptime subtracted the sum of the sectors
/// 
/// There may not be the same number of sectors each lap, as the format allows for this
/// 
/// This will always return at least one lap, which will be as long as timeperiode for which the channel had samples
pub fn get_laps_from_beacon(beacon_meta: &ChannelMetadata, beacon_data: &Vec<Sample>) -> Vec<Lap> {
    let mut laps = Vec::<Lap>::new();

    match (&beacon_meta.channel_feature_flag, &beacon_meta.datatype) {
        (ChannelFlag::Beacon, Datatype::Beacon16 | Datatype::Beacon32) => (),
        // in case a none beacon gets passed in we still return the length of the time periode stored in the channel as a lap
        _ => return vec![Lap { lap_time:((beacon_meta.data_count as i32) * 1000) / (beacon_meta.sample_rate as i32), sectors: Vec::<i32>::new() }],
    };

    // using i32 to store our timestamps in ms might cause issues... that is if the log is longer then ~596.5h, so as long as we don't bump the precision, we are fine
    let mut timestamps = Vec::<(i32, bool)>::new(); 
    let mut index = 0;

    let mut _last_low = i32::MIN;
    let (mut last_normal_index,mut _last_normal_value) = (0,0);

    for s in beacon_data {
        let s = match s {
            Sample::I16(v) => *v as i32,
            Sample::I32(v) => *v as i32,
            _ => 0
        };

        // Data is encoded in dual directional spikes
        // First: the value falls away from normal to a resting spot
        //      the transponder detected a new lap, but data from the previous lap is not available yet
        //      although in simulator logs it usually is already, but doesn't matter
        //      the value to which it falls can be variable or fixed, but is significantly lower then low, and lower then peak
        // Second: it falls to the low
        //      this goes down on the first round to i16::MIN + 1 (at least with i16 Beacon), although other values can be observed
        //      however the next spike always at minimum increments by 1 (new lap)
        //      if it increments by 2 this usually suggest a sector split, but the MoTeC Rally Sample increments by 6 for splits, which MoTeC takes incorrectly as new laps
        //      also the normal following seems to determine if it was a split or not
        // Third: rise to specific value
        //      this value is the offset from the lap (or sector) start after the last normal*
        //      the value is encoded as offset + 0x4000 = value (at least with a i16 Beacon, idk about i32 Beacon)
        //      offset is in ms (at least with i16)
        //      * last normal at a full second... so if a samplerate above 1hz is used we first divide through smaplerate to fix this... just why did they do this?
        // Forth: Return to normal
        //      A value of 100 seems to indicate a new lap, a value of 56 a new sector
        //      outlap starts at 0, will go to 56 for sectors (if they are registering on outlap)
        //      Number of sectors, or if any are included can be changed on a lap to lap basis
        //      In general sectors are not really used by MoTeC as far as I know and seem to be just ignored
        //      Rally sample has normal at 2 consitently

        if s > (i8::MIN as i32) && s < 0x4000 { // from testing upwards moves lower then 0x4000 are not counted as spikes
            // Within the none spike normal envolope
            if last_normal_index != index - 1 {
                // We are coming back from a flank
                if s == 56 && !timestamps.is_empty() {
                    let target = timestamps.len().clone() - 1;
                    timestamps[target].1 = true;
                }
            }

            (last_normal_index, _last_normal_value) = (index, s);
        } else if s < (_last_low / 2) {
            // Max low flank
            // Currently lets ignore this and only read the next normal to determine type
        } else if s < 0 {
            // First step down
            // We ignore it
        } else {
            // Upwards value, we extract offset, add a new timestamp with the determined time
            let offset = match beacon_meta.datatype {
                Datatype::Beacon16 => s - 0x4000,
                Datatype::Beacon32 => s - 0x4000, // We assume this is how it works
                // however come to think of it, maybe i32 exists to add precision TODO
                _ => 0, // this should not happen
            };

            let mut timestamp_ms = (last_normal_index / (beacon_meta.sample_rate as i32)) * 1000;
            //timestamp_ms /= beacon_meta.sample_rate as i32;
            timestamp_ms += offset;

            if let Some((time, _sector)) = timestamps.last() {
                // Sometimes peaks consist out of 2 or more samples.
                // But the samples evaluate to the same timestamp, so we can filter them out this way
                if time.clone() != timestamp_ms {
                    timestamps.push((timestamp_ms, false));
                }
            } else {
                timestamps.push((timestamp_ms, false));
            }
        } 


        index += 1;
    }

    // We have gathered the timestamps, now convert them into laps
    let mut sectors = Vec::<i32>::new();

    let mut last_timestamp = 0;
    let mut last_sector_timestamp = 0;

    for (t, is_sector) in timestamps {
        if is_sector {
            sectors.push(t - last_sector_timestamp);
            last_sector_timestamp = t;
        } else {
            laps.push(Lap{ lap_time: t - last_timestamp, sectors: sectors.clone() });
            sectors.clear();

            last_timestamp = t;
            last_sector_timestamp = last_timestamp;
        }
    }

    // Adding inlap
    let mut timestamp_ms = index * 1000;
    timestamp_ms /= beacon_meta.sample_rate as i32;
    laps.push(Lap{ lap_time: timestamp_ms - last_timestamp, sectors });


    laps
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Cursor};

    use crate::{ LDReader, Datatype, ChannelFlag };

    use super::*;

    // These tests will fail if reader tests fail
    
    #[test]
    fn find_sample1_beacon_channel() {
        let bytes = fs::read("./samples/Sample1.ld").unwrap();
        let mut cursor = Cursor::new(bytes);
        let mut reader = LDReader::new(&mut cursor);

        let channels = reader.read_channels().unwrap();
        let result = get_beacon_channel(&channels);

        let reference = ChannelMetadata {
            prev_addr: 16236,
            next_addr: 16484,
            data_addr: 301812,
            data_count: 454,
            datatype: Datatype::Beacon16,
            sample_rate: 1,
            offset: 0,
            mul: 1,
            scale: 1,
            dec_places: 0,
            name: "Beacon".to_string(),
            short_name: "Beacon".to_string(),
            unit: String::new(),
            channel_feature_flag: ChannelFlag::Beacon,
        };
        assert_eq!(result, Some(&reference));
    }

    #[test]
    fn get_sample1_laps() {
        let bytes = fs::read("./samples/Sample1.ld").unwrap();
        let mut cursor = Cursor::new(bytes);
        let mut reader = LDReader::new(&mut cursor);

        let channels = reader.read_channels().unwrap();
        let beacon = get_beacon_channel(&channels).unwrap();
        let beacon_data = reader.channel_data(beacon).unwrap();

        let result = get_laps_from_beacon(beacon, &beacon_data);
        
        assert_eq!(result,vec![
            Lap { lap_time: 94136, sectors: vec![43547,36838], },
            Lap { lap_time: 65163, sectors: vec![18894,32856], },
            Lap { lap_time: 63682, sectors: vec![18301,31888], },
            Lap { lap_time: 65192, sectors: vec![19027,32806], },
            Lap { lap_time: 63759, sectors: vec![18331,31666], },
            Lap { lap_time: 102068,sectors: vec![21935,33752], },
        ]);
    }

    #[test]
    fn get_laps_no_beacon() {
        let channel0_meta = ChannelMetadata {
            prev_addr: 0,
            next_addr: 0,
            data_addr: 0,
            data_count: 16,
            datatype: Datatype::I16,
            sample_rate: 2,
            offset: 0,
            mul: 1,
            scale: 1,
            dec_places: 1,
            name: "Air Temp Inlet".to_string(),
            short_name: "Air Tem".to_string(),
            unit: "C".to_string(),
            channel_feature_flag: ChannelFlag::Default
        };
        let channel0_samples = vec![
            Sample::I16(190),
            Sample::I16(190),
            Sample::I16(190),
            Sample::I16(190),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(200),
            Sample::I16(190),
            Sample::I16(190),
            Sample::I16(190),
        ];

        let result = get_laps_from_beacon(&channel0_meta, &channel0_samples);
        assert_eq!(result, vec![Lap {lap_time: 8000, sectors: Vec::<i32>::new()}]);
    }
}