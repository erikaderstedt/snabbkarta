use super::dtm::{DigitalTerrainModel,Terrain};
use std::path::Path;
use std::fs;
use super::ffi_helpers::{write_instance,write_instances,ftell,fseek};
use super::geometry::{Rectangle,LineSegment};
use super::Sweref;
use std::io::{Read,Write};
use colored::*;
use indicatif::ProgressBar;
use std::thread;
use std::sync::mpsc::{channel,Receiver,Sender};
use std::sync::Arc;

#[repr(C, packed)]
struct RekInfo {
    angle: f64,
    west: f64,
    east: f64,
    south: f64,
    north: f64,
    mid_x: f64,
    mid_y: f64,

    chunk_size: f64,

    chunks_x: u32,
    chunks_y: u32,

    num_points: u32,
    ocad_file_start: u32,
}

#[derive(Debug)]
struct ThreadResult {
    x: u32,
    y: u32,
    triangles: Vec<RawTriangle>,
}

#[repr(C, packed)]
#[derive(Copy,Clone,Debug)]
struct RawChunkStart {
    start: u32,
    num_triangles: u32,
}


#[repr(C, packed)]
struct RawPoint {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C, packed)]
#[derive(Copy,Clone,Debug)]
struct RawTriangle {
    p0: u32, p1: u32, p2: u32, 
    terrain: u32, uid: u32,
}

const CHUNK_SIZE: f64 = 75.0;

pub fn save_dtm_to_rek(dtm: &DigitalTerrainModel, 
    angle: f64,
    ocad_path: &Path,
    path: &Path) {

    let module = "OUT!".red();

    // Sweref coordinates
    let max_x = dtm.points.iter().map(|p| p.x).fold(0./0., f64::max);
    let min_x = dtm.points.iter().map(|p| p.x).fold(0./0., f64::min);
    let max_y = dtm.points.iter().map(|p| p.y).fold(0./0., f64::max);
    let min_y = dtm.points.iter().map(|p| p.y).fold(0./0., f64::min);
    let mid_x = 0.5f64 * (max_x + min_x);
    let mid_y = 0.5f64 * (max_y + min_y);

    let chunks_x = f64::ceil((max_x - min_x) / CHUNK_SIZE) as u32;
    let chunks_y = f64::ceil((max_y - min_y) / CHUNK_SIZE) as u32;

    let mut rek_info = RekInfo { 
        angle: angle,
        west: min_x,
        east: max_x,
        south: min_y,
        north: max_y,
        mid_x: mid_x,
        mid_y: mid_y,
        chunk_size: CHUNK_SIZE,
        chunks_x: chunks_x,
        chunks_y: chunks_y,
        num_points: dtm.points.len() as u32,
        ocad_file_start: 0u32,
    };

    let mut file = fs::File::create(path.to_str().expect("Unable to convert output path to str.")).expect("Unable to create output file.");
    write_instance(&rek_info, &mut file).expect("Unable to write REK header.");

    let chunk_table_start = ftell(&mut file);
    let num_chunks = (chunks_x * chunks_y) as usize;
    let mut chunk_starts = vec![ RawChunkStart { start: 0u32, num_triangles: 0u32 } ;num_chunks];
    write_instances(&chunk_starts, &mut file).expect("Unable to write dummy chunk table.");

    {
        let pointvalues = dtm.points.iter().map(|p| 
            RawPoint { 
                x: (p.x - mid_x) as f32,
                y: (p.y - mid_y) as f32,
                z: p.z as f32 }).collect();
        write_instances(&pointvalues, &mut file).expect("Unable to write points.");
    }

    println!("[{}] Creating segments and bounding boxes.", &module);

    let pts: Arc<Vec<[Sweref;3]>> = Arc::new((0..dtm.num_triangles).map(|triangle| {
        let p0: Sweref = (&dtm.points[dtm.vertices[triangle*3+0]]).into();
        let p1: Sweref = (&dtm.points[dtm.vertices[triangle*3+1]]).into();
        let p2: Sweref = (&dtm.points[dtm.vertices[triangle*3+2]]).into();
        [p0,p1,p2]
    }).collect());

    let vertices: Arc<Vec<[usize;3]>> = Arc::new((0..dtm.num_triangles).map(|triangle| {
        [dtm.vertices[triangle*3+0],dtm.vertices[triangle*3+1],dtm.vertices[triangle*3+2]]
    }).collect());

    let segments: Arc<Vec<[LineSegment;3]>> = Arc::new((0..dtm.num_triangles).map(|triangle| {
        let p0: Sweref = pts[triangle][0];
        let p1: Sweref = pts[triangle][1];
        let p2: Sweref = pts[triangle][2];

        [LineSegment::create(&p0, &p1), LineSegment::create(&p1, &p2), LineSegment::create(&p2, &p0)]
    }).collect());

    let bboxes: Arc<Vec<Rectangle>> = Arc::new((0..dtm.num_triangles).map(|triangle| {
        let p0: Sweref = pts[triangle][0];
        let p1: Sweref = pts[triangle][1];
        let p2: Sweref = pts[triangle][2];
        let min_x = f64::min(p0.east,f64::min(p1.east,p2.east));
        let max_x = f64::max(p0.east,f64::max(p1.east,p2.east));
        let min_y = f64::min(p0.north,f64::min(p1.north,p2.north));
        let max_y = f64::max(p0.north,f64::max(p1.north,p2.north));
        Rectangle::create(min_x, min_y, max_x, max_y)
    }).collect());

    println!("[{}] Creating {} chunk threads for {} chunks.", &module, chunks_y, num_chunks);

    let (tx, rx): (Sender<Arc<ThreadResult>>, Receiver<Arc<ThreadResult>>) = channel();
    let mut threads = Vec::new();
    let num_triangles = dtm.num_triangles;

    // Launch one thread per y.
    // Set up a receiving channel for x,y,Vec<RawTriangle>
    for y in 0..chunks_y {
        let tx_clone = tx.clone();
        let terrain = dtm.terrain.clone();
        let bboxes = bboxes.clone();
        let segments = segments.clone();
        let vertices = vertices.clone();
        let pts = pts.clone();

        threads.push(thread::spawn(move || {
            for x in 0..chunks_x {
                let rect = Rectangle::create(
                    CHUNK_SIZE * (x as f64) + min_x,
                    CHUNK_SIZE * (y as f64) + min_y,
                    CHUNK_SIZE * ((x+1) as f64) + min_x,
                    CHUNK_SIZE * ((y+1) as f64) + min_y);
    
                let border_segments = rect.segments();
                
                let triangles: Vec<usize> = (0..num_triangles).filter(|t| {
                    let bbox = bboxes[*t];
                    if bbox.max_x() < rect.min_x() || 
                        bbox.max_y() < rect.min_y() ||
                        bbox.min_y() > rect.max_y() ||
                        bbox.min_x() > rect.max_x() { return false }
                        let p0: Sweref = pts[*t][0];
                        let p1: Sweref = pts[*t][1];
                        let p2: Sweref = pts[*t][2];
                    rect.contains(&p0) || rect.contains(&p1) || rect.contains(&p2) ||
                    segments[*t].iter().any(|s| border_segments.iter().any(|j| s.intersection_with(j).is_some()) )
                }).collect();

                tx_clone.send(Arc::new(ThreadResult {
                    x: x,
                    y: y,
                    triangles: triangles.into_iter().map(|t| 
                        RawTriangle {
                            p0: vertices[t][0] as u32,
                            p1: vertices[t][1] as u32,
                            p2: vertices[t][2] as u32,
                            terrain: match terrain[t] {
                                Terrain::Unclassified => 0,
                                Terrain::Lake => 1,
                                Terrain::Cliff => 2,
                            },
                            uid: t as u32,
                        }).collect(),
                })).expect("Unable to pass chunk back to main thread.");
            }
        }));
    }

    let mut total_written = 0usize;
    let bar = ProgressBar::new(num_chunks as u64);
    for _ in 0..num_chunks {
        let result = rx.recv().expect("Unable to receive chunk!");
        let chunk_index = result.y * chunks_x + result.x;
        chunk_starts[chunk_index as usize] = RawChunkStart {
            start: ftell(&mut file),
            num_triangles: result.triangles.len() as u32,
        };
        write_instances(&result.triangles, &mut file).expect("Unable to write the chunk triangles.");
        total_written = total_written + result.triangles.len();
        bar.inc(1);
    }
    bar.finish();

    for t in threads.into_iter() {
        t.join().expect("Unable to join one of the chunk threads.");
    }
        
    println!("[{}] Wrote {} triangles.", &module, total_written);

    // Append OCD file
    rek_info.ocad_file_start = ftell(&mut file);
    let mut ocad_file = fs::File::open(ocad_path.to_str().expect("Bad ocad path")).expect("Unable to open OCAD file.");
    let mut ocad_file_data: Vec<u8> = Vec::new();
    ocad_file.read_to_end(&mut ocad_file_data).expect("Unable to read OCAD file.");
    file.write_all(&ocad_file_data).expect("Unable to append OCAD file data to REK file.");

    // println!("Writing {} bytes of ocad data at {}.", ocad_file_data.len(), rek_info.ocad_file_start);

    fseek(&mut file, 0u32);
    write_instance(&rek_info, &mut file).expect("Unable to write REK header.");

    fseek(&mut file, chunk_table_start);
    write_instances(&chunk_starts, &mut file).expect("Unable to rewrite chunk table.");
}
