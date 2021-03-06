use super::ocad;
use super::ffi_helpers::{read_instance,read_instances};
use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;
use super::Sweref;
use super::geometry;
use colored::*;
use dbase;

pub trait SurveyAuthorityConfiguration {
    fn supports_file(&self, base_filename: &str) -> bool;
    fn symbols_for_record(&self, base_filename: &str, dbase_record: &dbase::Record) -> Vec<ocad::GraphSymbol>;
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
    _header: ShapefileHeader,
    pub shape_type: ShapeType,
}

impl Iterator for Shapefile {
    type Item = (Vec<Vec<Sweref>>, geometry::Rectangle);

    fn next(&mut self) -> Option<(Vec<Vec<Sweref>>, geometry::Rectangle)> {
        
        let _record_header: ShapefileRecordHeader = match read_instance(&mut self.shp) {
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
            3 => Some(Shapefile { shp: file, _header: header, shape_type: ShapeType::Polyline, }),
            5 => Some(Shapefile { shp: file, _header: header, shape_type: ShapeType::Polygon, }),
            _ => None,
        }
    }
}

#[derive(Copy,Clone)]
enum ShapeType {
    Polygon,
    Polyline,
}

pub fn load_shapefiles<T: SurveyAuthorityConfiguration>(bounding_box: &geometry::Rectangle, 
    folder: &Path,
    authority: &T,
    file: &Sender<ocad::Object>, verbose: bool) {
        
    let module = "SHP".yellow();

    let input_files = fs::read_dir(folder)
        .expect("Unable to open shapefile folder!")
        .filter_map(|x| {
            let path = x.expect("Unable to read shapefile path.").path();
            match path.file_stem() {
                Some(s) if 
                    !path.is_dir() && path.to_str().unwrap().ends_with("shp") &&
                    authority.supports_file(s.to_str().expect("Bad path!")) => Some(path.clone()),
                _ => None,
            }
        });

    let mut records = 0;
    for shp in input_files {
        let p = shp.with_extension("dbf");
        let mut reader = dbase::Reader::from_path(p).expect("Unable to open dBase-III file!");
        let base_filename = shp.file_stem().unwrap().to_str().expect("Path is not valid UTF-8!");
        if authority.supports_file(base_filename) {
            let shp_iter = Shapefile::new(&shp).expect("Unsupported shape type in shapefile");
            let _shape_type = shp_iter.shape_type;
            // Shape files and dbf files.
            // These are read in conjunction. 
            for (dbf_record, (point_lists, item_bbox)) in reader.iter_records().zip(shp_iter) {
                // println!("{:?}", item_bbox);
                if !bounding_box.intersects(&item_bbox) { continue; }

                let symbols = authority.symbols_for_record(base_filename, &dbf_record.expect("Unable to read DBF record."));
                ocad::post_objects(point_lists, &symbols, file, bounding_box);
                records = records + 1;
            }
        } 
    }

    if verbose {
        println!("[{}] Loaded {} records from shapefiles.", &module, records);
    }
}
