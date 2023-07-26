use crate::{I2Error, I2Result};

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Header {
    pub channel_meta_ptr: u32,
    pub channel_data_ptr: u32,
    pub event_ptr: u32,

    pub device_serial: u32,
    pub device_type: String,
    pub device_version: u16,

    pub num_channels: u32,

    // TODO: Replace with timestamp
    pub date_string: String,
    pub time_string: String,

    // TODO: Probably should be Option<String>?
    pub driver: String,
    pub vehicleid: String,
    pub venue: String,
    pub session: String,
    pub short_comment: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sample {
    I8(i8),
    I16(i16),
    I32(i32),
    F32(f32),
}

impl Sample {
    /// Calculates the final value of this sample as a f64
    pub fn decode_f64(&self, channel: &ChannelMetadata) -> f64 {
        let value = match self {
            Sample::I8(v) => *v as f64,
            Sample::I16(v) => *v as f64,
            Sample::I32(v) => *v as f64,
            Sample::F32(v) => *v as f64,
        };

        // TODO: Offset not yet supported
        //assert_eq!(channel.offset, 0);
        let value = value / channel.scale as f64;
        let value = value * (10.0f64.powi(-channel.dec_places as i32));
        let value = value * channel.mul as f64;
        value
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum Datatype {
    // TODO: Not Too sure about this data type, it shows up as beacon in the sample dataset
    // It behaves as an integer of the same size
    Beacon16,
    Beacon32,
    I8,
    I16,
    I32,

    F16,
    F32,

    Invalid,
}

impl Datatype {
    /// Size in bytes that this datatype occupies on file
    pub fn size(&self) -> u16 {
        match self {
            Datatype::I8 => 1,
            Datatype::Beacon16 | Datatype::I16 | Datatype::F16 => 2,
            Datatype::Beacon32 | Datatype::I32 | Datatype::F32 => 4,

            // We really don't know what these values are
            Datatype::Invalid => 0,
        }
    }

    pub fn _type(&self) -> u16 {
        match self {
            Datatype::Beacon16 | Datatype::Beacon32 => 0,
            Datatype::I8 | Datatype::I16 | Datatype::I32 => 3,
            Datatype::F16 | Datatype::F32 => 7,
            Datatype::Invalid => 999,
        }
    }

    pub fn from_type_and_size(_type: u16, size: u16) -> I2Result<Self> {
        match (_type, size) {
            (0, 2) => Ok(Datatype::Beacon16),
            (0, 4) => Ok(Datatype::Beacon32),
            (3, 1) => Ok(Datatype::I8), // From DAMP Plugin
            (3, 2) => Ok(Datatype::I16),
            (3, 4) => Ok(Datatype::I32),
            // 20160903-0051401.ld uses 5 for ints?
            (5, 2) => Ok(Datatype::I16),
            (5, 4) => Ok(Datatype::I32),
            (7, 2) => Ok(Datatype::F16),
            (7, 4) => Ok(Datatype::F32),

            // The mu iracing exporter exports these values on Damper Pos FL/FR/RL, they have 0 samples
            (17536, 5) | (6566, 5) | (29813, 5) => Ok(Datatype::Invalid),
            // This should be Beacon40 ?, but the iRacing mu exporter puts this in Damper Pos RR
            (0, 5) => Ok(Datatype::Invalid),
            // Iracing mu exporter Ride Height Center 0 samples
            (15, 5) => Ok(Datatype::Invalid),
            _ => Err(I2Error::UnrecognizedDatatype { _type, size }),
        }
    }
}

/// ChannelMetadata is a doubly linked list of blocks in the file
/// This only contains info about a channel, actual data is stored somewhere else on the file.
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct ChannelMetadata {
    pub prev_addr: u32,
    pub next_addr: u32,

    pub data_addr: u32,
    pub data_count: u32,

    pub datatype: Datatype,
    /// Sample Rate in Hz
    pub sample_rate: u16,

    pub offset: u16,
    pub mul: u16,
    pub scale: u16,
    pub dec_places: i16,

    pub name: String,
    pub short_name: String,
    pub unit: String,
}

impl ChannelMetadata {
    /// Size of a metadata entry in bytes
    pub(crate) const ENTRY_SIZE: u32 = 124;

    /// Calculates the size in bytes of the data section for this channel
    pub(crate) fn data_size(&self) -> u32 {
        self.data_count * self.datatype.size() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Event {
    /// Max 64 chars
    pub name: String,
    /// Max 64 chars
    pub session: String,
    /// Max 1024 chars
    pub comment: String,

    pub venue_addr: u32,

    pub weather_addr: u32, // TODO Add Weather data
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Venue {
    /// Max 64 chars
    pub name: String,
    /// Measured in mm, or 10^-3m (aka meter with 3 dec_place)
    pub length: i32,
    /// Shows up as Venue Best Lap under Custom underneath Vehicle in the Details
    /// Measured in ms
    pub venue_best_lap: i32,

    pub vehicle_addr: u32,
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Vehicle {
    /// Max 64 chars
    pub id: String,
    /// Max 64 chars
    pub desc: String,
    /// Max 64 chars
    pub engine_id: String,
    /// Measured in kg
    pub weight: i16,
    /// Measured in 10^(-1)liter (aka 1 dec_place)
    pub fuel_tank_i16: i16,
    /// Max 32 chars
    pub _type: String,
    /// Max 32 chars
    pub drive_type: String,
    /// Max 1024 chars
    pub comment: String,
    /// Gear Ratios, stored as a 3 dec_places value
    /// index 0 is diff (final drive), index 1-10 are Gears 1-10
    pub gear_ratios_i16: [i16;11],
    /// Track width, messured in mm
    pub track_width: i16,
    /// Wheelbase, messured in mm
    pub wheel_base: i16
}

impl Vehicle {

    /// Retrieves the real values for the gear ratios
    /// Diff (Final drive) is Index 0, Index 1-11 is Gear 1-11
    /// If gear ratio is 0 (treated as none by MoTeC) it returns NaN
    pub fn get_float_ratios(& self) -> [f64;11] {
        let mut ratios = [f64::NAN;11];
        let mut index = 0;
        for r in self.gear_ratios_i16 {
            if r != 0 {
                ratios[index] = r as f64;
                ratios[index] *= 10.0f64.powi(-3); 
            }
            
            index += 1;
        }

        ratios
    }

    /// Simpler function for writing gear ratios
    /// Diff (Final drive) is Index 0, Index 1-11 is Gear 1-11
    /// All index not included in the vector will be set to 0 (treated as none by MoTeC)
    pub fn with_float_ratios(mut self, ratios: Vec<f64>) -> Self {
        self.gear_ratios_i16 = [0_i16;11]; 

        let mut index = 0;
        for r in ratios {
            if index >= 10 {
                break; // in case someone gave us an array that is larger then 10
            }

            if r.is_normal() { // if NaN or 0 the gear does not exist
                let r = r * (10.0f64.powi(3));
                self.gear_ratios_i16[index] = r as i16;
            }

            index += 1;
        }

        self
    }

    /// Returns the fuel tank size in liters
    pub fn get_fuel_tank_size(& self) -> f64 {
        let tank = self.fuel_tank_i16 as f64;
        tank * (10.0f64.powi(-1))
    }

    /// Simpler function for writing the fueltank size in liters
    pub fn with_fuel_tank_size(mut self, fuel_tank: f64) -> Self {
        let fuel_tank = fuel_tank * (10.0f64.powi(1)) + 0.5; // +0.5 serves to round the number correctly instead of just cropping
        self.fuel_tank_i16 = fuel_tank as i16;

        self
    }
}
