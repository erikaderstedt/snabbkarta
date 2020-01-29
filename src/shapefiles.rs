extern crate dbase;

use super::ocad;
use super::ffi_helpers::{read_instance,read_instances};
use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;
use super::sweref_to_wgs84::Sweref;
use super::geometry;
use colored::*;

// https://www.lantmateriet.se/globalassets/kartor-och-geografisk-information/kartor/fastshmi.pdf

enum LantmaterietShapeSymbol {
    KL, // Linjeskikt med kraftledningar
    VL, // Linjeskikt med vägar
    VO, // Linjeskikt med övriga vägar
    HL, // Linjeskikt med hydrografi
    ML, // Linjeskikt med markdata

    MY, // Ytskikt med heltäckande markdata
    MB, // Ytskikt med bebyggelse
    MS, // Ytskikt med sankmark
}

impl LantmaterietShapeSymbol {
    fn from_str(s: &str) -> Option<Self> {
        match &s[0..2] {
            "vl" => Some(Self::VL),
            "kl" => Some(Self::KL),
            "vo" => Some(Self::VO),
            "hl" => Some(Self::HL),
            "ml" => Some(Self::ML),
            "my" => Some(Self::MY),
            "mb" => Some(Self::MB),
            "ms" => Some(Self::MS),
            _ => None,
        }
    }
}

#[repr(C,packed)]
struct ShapefileHeader {
    file_code: u32,
    reserved: [u32;5],
    file_length: u32,
    version: u32,
    shape_type: u32,
    unused: [f64;8],
}

#[repr(C,packed)]
struct ShapefileRecordHeader {
    record_number: u32,
    content_length: u32,
}

#[repr(C,packed)]
struct ShapefilePoly {
    shape_type: u32,
    xmin: f64,
    ymin: f64,
    xmax: f64,
    ymax: f64,
    num_parts: u32,
    num_points: u32,
}

#[repr(C,packed)]
struct ShapefilePoint {
    x: f64,
    y: f64,
}

struct Shapefile {
    shp: fs::File,
    header: ShapefileHeader,
    pub shape_type: ShapeType,
}

impl Iterator for Shapefile {
    type Item = (Vec<Vec<Sweref>>, geometry::Rectangle);

    fn next(&mut self) -> Option<(Vec<Vec<Sweref>>, geometry::Rectangle)> {
        
        let record_header: ShapefileRecordHeader = match read_instance(&mut self.shp) {
            Ok(h) => h,
            Err(_) => { return None },
        };

        let poly: ShapefilePoly = read_instance(&mut self.shp).expect("Unable to load polygon header in shapefile.");
        
        let mut part_starts: Vec<u32> = read_instances(&mut self.shp, poly.num_parts as usize).expect("Unable to load parts.");
        part_starts.push(poly.num_points);
        
        let mut points: Vec<Vec<Sweref>> = Vec::new();
        for i in 0..(poly.num_parts as usize) {
            let num_points_in_part = (part_starts[i+1] - part_starts[i]) as usize;
            let points_in_part: Vec<ShapefilePoint> = read_instances(&mut self.shp, num_points_in_part).expect("Unable to load points for object in shapefile.");

            points.push(points_in_part.iter().map(|p| Sweref { east: p.x, north: p.y, }).collect());
        }
        Some((points, geometry::Rectangle::create(poly.xmin, poly.ymin, poly.xmax, poly.ymax)))
    }
}

impl Shapefile {
    fn new(path: &Path) -> Option<Shapefile> {
        let mut file = fs::File::open(path).expect("Unable to open shapefile");
        let header: ShapefileHeader = read_instance(&mut file).expect("Unable to read shapefile header.");

        match header.shape_type {
            3 => Some(Shapefile { shp: file, header: header, shape_type: ShapeType::Polyline, }),
            5 => Some(Shapefile { shp: file, header: header, shape_type: ShapeType::Polygon, }),
            _ => None,
        }
    }
}

#[derive(Copy,Clone)]
enum ShapeType {
    Polygon,
    Polyline,
}

pub fn load_shapefiles(bounding_box: &geometry::Rectangle, 
    folder: &Path,
    file: &Sender<ocad::Object>, verbose: bool) {
        
    let module = "SHP".yellow();

    let input_files = fs::read_dir(folder)
        .expect("Unable to open shapefile folder!")
        .filter_map(|x| {
        let path = x.expect("Unable to read shapefile path.").path();
        match !path.is_dir() && path.ends_with(".shp") {
            true => Some((path.clone(), path.with_extension("dbf"))),
            false => None,
        }
    });


    let mut records = 0;
    for (shp, dbf) in input_files {
        let p = &dbf;
        let mut reader = dbase::Reader::from_path(p).expect("Unable to open dBase-III file!");
        if let Some(symbol) = LantmaterietShapeSymbol::from_str(p.to_str().expect("Path is not valid UTF-8!")) {
            let shp_iter = Shapefile::new(&shp).expect("Unsupported shape type in shapefile");
            let shape_type = shp_iter.shape_type;
            // Shape files and dbf files.
            // These are read in conjunction. 
            for (dbf_record, (point_lists, item_bbox)) in reader.iter_records().zip(shp_iter) {
                if !bounding_box.intersects(&item_bbox) { continue; }

                let r = dbf_record.expect("Unable to read DBF record.");
                let v = match r.get("DETALJTYP").expect("No DETALJTYP field in record.") { 
                    dbase::FieldValue::Character(s) => s.as_ref(),
                    _ => panic!("Invalid field value for DETALJTYP field."),
                };

                records = records + 1;

                // let 

                // // For polygons, several parts make up a block. Each clockwise part makes up the start of a polygon. So, only begin a new ocad object when 

                // match symbol {
                //     LantmaterietShapeSymbol::MS |   LantmaterietShapeSymbol::MB | LantmaterietShapeSymbol::MY => {

                //     }
                //     LantmaterietShapeSymbol::HL | LantmaterietShapeSymbol::KL | 
                    
                //     polyline(symbol, &record),
                //     _ => polygon(symbol, &record),
                // }
    
            }
        } 
    }

    if verbose {
        println!("[{}] Loaded {} records from shapefiles.", &module, records);
    }
}
