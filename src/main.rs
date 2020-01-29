extern crate getopts;
extern crate delaunator;
extern crate colored;

use getopts::Options;
use std::env;
use std::path::Path;
use delaunator::{Point, triangulate};
use std::f64;
use std::f64::consts::PI;
use colored::*;
use std::thread;
use std::sync::mpsc::{channel,Receiver,Sender};

mod las;
mod sweref_to_wgs84;
mod wmm;
mod osm;
mod ocad;
mod geometry;
mod shapefiles;
mod ffi_helpers;
use sweref_to_wgs84::{Sweref,Wgs84};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] <las file> [...<las file>]*", program);
    print!("{}", opts.usage(&brief));
}

#[derive(Copy,Clone)]
struct Point3D {
    x: f64,
    y: f64,
    z: f64,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let module = "MAIN".green();

    let mut opts = Options::new();
    opts.optflag("v", "verbose", "show additional information while running");
    opts.optflag("s", "shapefiles", "path to a folder containing Lantmäteriet shapefiles.");
    opts.optflag("h", "help", "show this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let verbose = matches.opt_present("v");
    if matches.free.is_empty() {
        print_usage(&program, opts);
        return;
    }

    let shp_path = matches.opt_str("s");

    let appname = match verbose { false => "Snabbkarta", true =>
r#"   _____             __    __    __              __       
  / ___/____  ____ _/ /_  / /_  / /______ ______/ /_____ _
  \__ \/ __ \/ __ `/ __ \/ __ \/ //_/ __ `/ ___/ __/ __ `/
 ___/ / / / / /_/ / /_/ / /_/ / ,< / /_/ / /  / /_/ /_/ / 
/____/_/ /_/\__,_/_.___/_.___/_/|_|\__,_/_/   \__/\__,_/  
                                                          "# };

    println!("{} {}\n(c) Autopercept 2018-2020.\nContact: erik.aderstedt@autopercept.com\n", appname, VERSION);

    // Input in matches.free
    let f = matches.free[0].clone();
    let output_path = Path::new(&f).with_extension("ocd");
    
    let headers: Vec<las::LAS_File_Header> = matches.free.iter().map(|x| las::LAS_File_Header::new(Path::new(&x))).collect();
    let total_number_of_records = headers.iter().fold(0, |num_records, header| num_records + header.number_of_point_records);
    let max_x = headers.iter().map(|x| x.max_x).fold(0./0., f64::max);
    let min_x = headers.iter().map(|x| x.min_x).fold(0./0., f64::min);
    let max_y = headers.iter().map(|x| x.max_y).fold(0./0., f64::max);
    let min_y = headers.iter().map(|x| x.min_y).fold(0./0., f64::min);
    let max_z = headers.iter().map(|x| x.max_z).fold(0./0., f64::max);
    let min_z = headers.iter().map(|x| x.min_z).fold(0./0., f64::min);

    if verbose { println!("[{}] Writing to {:?}", &module, output_path); }

    let x_scale_factor = headers[0].x_scale_factor;
    let x_offset = headers[0].x_offset;
    let y_scale_factor = headers[0].y_scale_factor;
    let y_offset = headers[0].y_offset;
    let z_scale_factor = headers[0].z_scale_factor;
    let z_offset = headers[0].z_offset;

    let height_over_sea_level: f64 = (max_z + min_z)*0.5;
    let bounding_box = geometry::Rectangle { southwest: Sweref { north: min_y, east: min_x, }, northeast: Sweref { north: max_y, east: max_x, }};
    let middle_of_map = Wgs84::from_sweref( &bounding_box.middle() );
    let top_of_map = Sweref::from_wgs84(
        &Wgs84 { latitude: middle_of_map.latitude + 0.003, longitude: middle_of_map.longitude});
    let bottom_of_map = Sweref::from_wgs84(
        &Wgs84 { latitude: middle_of_map.latitude - 0.003, longitude: middle_of_map.longitude});
    let meridian_convergence: f64 = 90.0f64 - f64::atan2(top_of_map.north-bottom_of_map.north, top_of_map.east - bottom_of_map.east)*180f64/PI;
    let magnetic_declination: f64 = wmm::get_todays_magnetic_declination(&middle_of_map, height_over_sea_level);
    let northeast_corner = Wgs84::from_sweref(&bounding_box.northeast);
    let southwest_corner = Wgs84::from_sweref(&bounding_box.southwest);

    if verbose {
        println!("[{}] Average height over sea level: {:.0} m", &module, height_over_sea_level);
        println!("[{}] Meridian convergence: {:.2}°", &module, meridian_convergence);
        println!("[{}] Magnetic declination: {:.2}°", &module, magnetic_declination);

        println!("[{}] https://maps.apple.com/?ll={:.5},{:.5}&t=k&spn={:.5},{:.5}", &module, 
            middle_of_map.latitude, 
            middle_of_map.longitude,
            northeast_corner.latitude - southwest_corner.latitude,
            northeast_corner.longitude - southwest_corner.longitude);
        println!("[{}] https://www.google.com/maps/@?api=1&map_action=map&center={:.5},{:.5}&zoom=14&basemap=satellite", &module,
            middle_of_map.latitude,
            middle_of_map.longitude);
    }

    // Set up OCAD file, with two receive channels. 
    // Start OSM curl.
    // 

    let (ocad_tx, ocad_rx): (Sender<ocad::Object>, Receiver<ocad::Object>) = channel();
    let ocad_thread = thread::spawn(move || {
        ocad::create(&output_path, 
            &bounding_box,  
            magnetic_declination + meridian_convergence, 
            &ocad_rx);
    });

    let tx = ocad_tx.clone();
    let preexisting_map_thread = thread::spawn(move || {
        match shp_path {
            None => { osm::load_osm(&southwest_corner, &northeast_corner, &tx, verbose); },
            Some(p) => { shapefiles::load_shapefiles(&bounding_box, &Path::new(&p), &tx, verbose); },
        }
    });

    let records: Vec<las::PointDataRecord> = matches.free.iter().map(|x| las::PointDataRecord::load_from(Path::new(&x))).flatten().collect();
    let ground_points: Vec<Point> = records.iter()
        .filter(|record| record.classification == 2)
        .map(|record| Point { x: ((record.x as f64) * x_scale_factor + x_offset) - min_x,
                        y: ((record.y as f64) * y_scale_factor + y_offset) - min_y })
        .collect();


    let to_point_3d = |record: las::PointDataRecord| Point3D {
        x: ((record.x as f64) * x_scale_factor + x_offset) - min_x,
        y: ((record.y as f64) * y_scale_factor + y_offset) - min_y,
        z: ((record.y as f64) * z_scale_factor + z_offset) - min_z,
    };

    let result = triangulate(&ground_points).expect("No triangulation exists.");
    println!("[{}] DTM triangulation complete, {:?} triangles", &module, result.len());

    preexisting_map_thread.join().expect("Unable to finish pre-existing map thread");
    ocad_tx.send(ocad::Object::termination()).expect("Unable to tell OCAD thread to finish");
    ocad_thread.join().expect("Unable to finish OCAD thread");

}
