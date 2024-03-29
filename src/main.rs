use getopts::Options;
use std::env;
use std::path::Path;
use std::f64;
use std::f64::consts::PI;
use colored::*;
use std::thread;
use std::sync::mpsc::{channel,Receiver,Sender};

mod las;
mod rek;
mod sweref; mod wgs84;
mod wmm;
mod osm;
mod ocad;
mod geometry;
mod shapefiles;
mod lantmateriet;
mod ffi_helpers;
mod lakes;
mod dtm;
mod boundary;
mod meridians;
mod cliffs;
mod contours;
mod ml_input_data;
mod hexgrid;

use sweref::Sweref;
use wgs84::Wgs84;
use geometry::{Point3D,PointConverter};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] <las file> [...<las file>]*", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let module = "MAIN".green();

    let mut opts = Options::new();
    opts.optflag("q", "quiet", "hide additional information while running");
    opts.optopt("s", "", "shapefiles", "path to a folder containing Lantmäteriet shapefiles.");
    opts.optflag("m", "ml-input-data", "create a .ml-input-data file instead of an OCAD file");
    opts.optflag("h", "help", "show this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }
    let verbose = !matches.opt_present("q");
    if matches.free.is_empty() {
        print_usage(&program, opts);
        return;
    }

    let shp_path = matches.opt_str("s");
    let create_ml_data = matches.opt_present("m");

    let appname = match verbose { false => "Snabbkarta", true => r#"
   _____             __    __    __              __       
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
    let max_x = headers.iter().map(|x| x.max_x).fold(0./0., f64::max);
    let min_x = headers.iter().map(|x| x.min_x).fold(0./0., f64::min);
    let max_y = headers.iter().map(|x| x.max_y).fold(0./0., f64::max);
    let min_y = headers.iter().map(|x| x.min_y).fold(0./0., f64::min);
    let max_z = headers.iter().map(|x| x.max_z).fold(0./0., f64::max);
    let min_z = headers.iter().map(|x| x.min_z).fold(0./0., f64::min);

    if verbose { println!("[{}] Writing to {:?}", &module, output_path); }

    let height_over_sea_level: f64 = min_z;
    let bounding_box = geometry::Rectangle { southwest: Sweref { north: min_y, east: min_x, }, northeast: Sweref { north: max_y, east: max_x, }};
    let middle_of_map = Wgs84::from( &bounding_box.middle() );
    let top_of_map = Sweref::from(
        &Wgs84 { latitude: middle_of_map.latitude + 0.003, longitude: middle_of_map.longitude});
    let bottom_of_map = Sweref::from(
        &Wgs84 { latitude: middle_of_map.latitude - 0.003, longitude: middle_of_map.longitude});
    let meridian_convergence: f64 = 90.0f64 - f64::atan2(top_of_map.north-bottom_of_map.north, top_of_map.east - bottom_of_map.east)*180f64/PI;
    let magnetic_declination: f64 = wmm::get_todays_magnetic_declination(&middle_of_map, height_over_sea_level*0.001);
    let northeast_corner = Wgs84::from(&bounding_box.northeast);
    let southwest_corner = Wgs84::from(&bounding_box.southwest);

    if verbose {
        println!("[{}] Lowest point over sea level: {:.0} m", &module, height_over_sea_level);
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

    let (ocad_tx, ocad_rx): (Sender<ocad::Object>, Receiver<ocad::Object>) = channel();
    let ocad_thread = thread::spawn(move || {
        ocad::create(&output_path, 
            &bounding_box,  
            magnetic_declination + meridian_convergence, 
            &ocad_rx);
    });

    let tx_preexisting = ocad_tx.clone();
    let preexisting_map_thread = thread::spawn(move || {
        match shp_path {
            None => { osm::load_osm(&southwest_corner, &northeast_corner, &tx_preexisting, verbose); },
            Some(p) => { shapefiles::load_shapefiles(&bounding_box, &Path::new(&p), &lantmateriet::LantmaterietShapes {}, &tx_preexisting, verbose); },
        }
    });

    let records: Vec<las::PointDataRecord> = matches.free.iter().map(|x| 
        las::PointDataRecord::load_from(Path::new(&x), x.ends_with(".laz")).expect("Unable to read point data records from LAS file.")
    ).flatten().collect();
    println!("[{}] {} point data records in {} files.", &module, records.len(), matches.free.len());

    let low_vegetation = records.iter().filter(|r| r.classification == 3u8).count(); 
    let medium_vegetation = records.iter().filter(|r| r.classification == 4u8).count(); 
    let high_vegetation = records.iter().filter(|r| r.classification == 5u8).count(); 

    let ground_points = records.iter().filter(|r| r.classification == 2u8).count(); 
    let water_points = records.iter().filter(|r| r.classification == 9u8).count(); 
    let building_points = records.iter().filter(|r| r.classification & 31 == 6u8).count(); 

    let unclassified = records.iter().filter(|r| r.classification == 1u8).count(); 


    println!("[{}] {} / {} / {} low / medium / high vegetation points.", &module, low_vegetation, medium_vegetation, high_vegetation);
    println!("[{}] {} ground and {} water points.", &module, ground_points, water_points);
    println!("[{}] {} building and {} unclassified points.", &module, building_points, unclassified);

    let point_converter = PointConverter::from(&headers[0]);


    let mut dtm = dtm::DigitalTerrainModel::create(&records, &point_converter);
    println!("[{}] DTM triangulation complete, {:?} triangles", &module, dtm.num_triangles);

    // let hex_grid = hexgrid::HexGrid::covering_bounds(&dtm.bounds, 1.2);
    // let ml_data = ml_input_data::MachineLearningInputData::construct_hashmap(&records, &point_converter, &dtm, &hex_grid);

    // println!("[{}] {} hex grid points generated.", &module, ml_data.len());

    // TODO: run cliffs / lakes in parallel. Hard to do when they both need mutable references
    // to the dtm.
    cliffs::detect_cliffs(&mut dtm, &ocad_tx, verbose);
    lakes::find_lakes(&records, &point_converter, &mut dtm, &ocad_tx, verbose);

    // Divide DTM into 50x50 m sections and save triangles, points. In blocks.

    // struct Block 
    //      file offset
    //      number of points
    //      number of triangles
    //      x_index
    //      y_index


    let contour_thread = {
        let tx_contours = ocad_tx.clone();
        let dtm_clone = dtm.clone();
        thread::spawn(move || {
            contours::create_contours(dtm_clone, min_z, max_z, point_converter.z_resolution(), tx_contours, verbose); })
    };

    meridians::add_meridians(&bounding_box, magnetic_declination+meridian_convergence, &ocad_tx, verbose);
    // //water_model::rain_on(&mut dtm, &ocad_tx, verbose);

    preexisting_map_thread.join().expect("Unable to finish pre-existing map thread.");

    contour_thread.join().expect("Unable to finish contour thread.");

    ocad_tx.send(ocad::Object::termination()).expect("Unable to tell OCAD thread to finish.");
    ocad_thread.join().expect("Unable to finish OCAD thread.");

    // // Load 
    let rek_output_path = Path::new(&f).with_extension("rek");

    rek::save_dtm_to_rek(&dtm, 
        meridian_convergence + magnetic_declination, 
        &Path::new(&f).with_extension("ocd"),
        &rek_output_path);
}