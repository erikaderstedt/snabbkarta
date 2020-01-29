use std;
use std::error::Error;
use std::io::prelude::*;
use std::path::Path;
use std::mem;
use std::slice;
use std::io::SeekFrom;
use std::io::Read;
use std::fs::File;
use super::ffi_helpers::{read_instance, read_instances};

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
            Err(why) => panic!("Couldn't open {}: {}", path.display(), why.description()),
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

impl PointDataRecord {
    pub fn load_from(path: &Path) -> Vec<PointDataRecord> {
        let header = LAS_File_Header::new(path);
        let mut file = File::open(path).unwrap(); 

        file.seek(SeekFrom::Start(header.offset_to_point_data.into())).unwrap();

        read_instances(&mut file, header.number_of_point_records as usize).expect("Unable to read point data records from LAS file.")
    } 
}


