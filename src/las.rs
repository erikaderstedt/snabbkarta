use std;
use std::io::prelude::*;
use std::path::Path;
use std::io::SeekFrom;
use std::fs::File;
use super::ffi_helpers::{read_instance, read_instances};
use std::convert::TryInto;
use laz;

#[repr(C, packed)]
pub struct LAS_File_Header {
    file_signature:  [u8; 4],
    file_source_id:   u16,
    global_encoding: u16,

    project_id_1:    u32,
    project_id_2:    u16,
    project_id_3:    u16,
    project_id_4:    [u8; 8],

    version_major:   u8,
    version_minor:   u8,
    system_identifier: [u8; 32],
    generating_software: [u8; 32],

    day_of_year_created:   u16,
    year_created:        u16,
    header_size:         u16,
    pub offset_to_point_data:  u32,
    number_of_variable_length_records:  u32,
    point_data_format_id: u8,
    point_data_record_length: u16,
    pub number_of_point_records: u32,
    number_of_points_by_return: [u32; 5],
    
    pub x_scale_factor: f64,
    pub y_scale_factor: f64,
    pub z_scale_factor: f64,
    pub x_offset: f64,
    pub y_offset: f64,
    pub z_offset: f64,

    pub max_x: f64, // m
    pub min_x: f64, // m
    pub max_y: f64, // m
    pub min_y: f64, // m
    pub max_z: f64, // m
    pub min_z: f64 // m
}

impl LAS_File_Header {
    pub fn new(path: &Path) -> LAS_File_Header {

        let mut file = match File::open(path) {
            Err(why) => panic!("Couldn't open {}: {:?}", path.display(), why),
            Ok(file) => file,
        };

        read_instance(&mut file).expect("Unable to read LAS file header.")
    }
}

#[repr(C, packed)]
pub struct PointDataRecord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    intensity: u16,
    ret: u8,
    pub classification: u8,
    scan_angle: i8,
    user_data: u8,
    point_source_id: u16,
    gps_time: f64,
}

#[repr(C, packed)]
struct Lasvlr {
    reserved: u16,
    userid: [u8; 16],
    record_id: u16,
    record_length_after_header: u16,
    description: [u8;32],
}


impl PointDataRecord {
    pub fn load_from(path: &Path, compressed: bool) -> std::io::Result<Vec<PointDataRecord>> {
        let mut file = File::open(path).unwrap();
        let header: LAS_File_Header = read_instance(&mut file).expect("Unable to read LAS file header.");

        if compressed {
            let number_of_points = header.number_of_point_records as usize;
            let _vlr_header: Lasvlr = read_instance(&mut file).expect("Unable to read vlr");
            let vlr = laz::LazVlr::read_from(&mut file).unwrap();            
            file.seek(SeekFrom::Start(header.offset_to_point_data.into())).unwrap();
        
            let mut buffer: Vec<u8> = Vec::new();
            // read the whole file
            file.read_to_end(&mut buffer).expect("Unable to read to end");
            // The offset to the chunk table (first i64) is relative to the beginning of the file, but we are passing
            // in just the point information. We need to subtract how much we've consumed up until now.
            let offset: i64 = i64::from_le_bytes(buffer[..8].try_into().expect("Buffer too short"));
            let amended_offset: i64 = offset - (header.offset_to_point_data as i64);
            let b = amended_offset.to_le_bytes();
            for j in 0..8 { buffer[j] = b[j]; }

            let output_size = number_of_points * (vlr.items_size() as usize);
            let mut output = vec![0u8; output_size];
            laz::las::laszip::decompress_buffer(&buffer, &mut output, vlr).expect("Unable to decompress");
        
            let mut c = std::io::Cursor::new(output);
            read_instances(&mut c, number_of_points)
        } else {
            file.seek(SeekFrom::Start(header.offset_to_point_data.into())).unwrap();
            read_instances(&mut file, header.number_of_point_records as usize)
        }
    } 
}


