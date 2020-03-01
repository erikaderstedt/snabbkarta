use super::dtm::{DigitalTerrainModel,Terrain,Point3D,Halfedge,TriangleWalk};
use delaunator::EMPTY;
use super::boundary::Boundary;
use std::f64;
use super::ocad;
use std::sync::mpsc::Sender;
use colored::*;
use super::Sweref;

const ABSORBTION_FACTOR: f64 = 0.151f64;
const RAIN_MM: f64 = 10f64;
const RAIN_M: f64 = RAIN_MM*0.001;
const ITERATE_UNTIL_ONLY_THIS_MUCH_WATER_REMAINS: f64 = 5f64; // cubic m

// The overlap is intentional.
const DIFFUSE_MARSH_LOWER_LIMIT: f64 = RAIN_M*2.0;
const DIFFUSE_MARSH_UPPER_LIMIT: f64 = RAIN_M*5.0;

const NORMAL_MARSH_LOWER_LIMIT: f64 = RAIN_M*4.0;
const NORMAL_MARSH_UPPER_LIMIT: f64 = RAIN_M*10.0;

const IMPASSABLE_MARSH_LOWER_LIMIT: f64 = RAIN_M*7.0;

const MIN_AREA_FOR_SEED: f64 = 0.5f64;

#[derive(Debug)]
enum Marsh {
    Diffuse,
    Normal,
    Impassable,
}

impl Marsh {
    fn symbol(&self) -> i32 { match self {
        // Self::Diffuse => 310000,
        // Self::Normal => 308000,
        // Self::Impassable => 307000,
        Self::Diffuse => 214000,
        Self::Normal => 406000,
        Self::Impassable => 408000,
    }}

    fn limits(&self) -> (f64, f64) { match self {
        Self::Diffuse => (DIFFUSE_MARSH_LOWER_LIMIT, DIFFUSE_MARSH_UPPER_LIMIT),
        Self::Normal => (NORMAL_MARSH_LOWER_LIMIT, NORMAL_MARSH_UPPER_LIMIT),
        Self::Impassable => (IMPASSABLE_MARSH_LOWER_LIMIT, f64::MAX),
    }}
}

// In Lantmäteriet data, the overlap region between two flight paths offer a large number of small completely flat
// triangles, owing to the higher point density in these areas. We must ensure that water keeps flowing past these triangles.


pub fn rain_on(dtm: &mut DigitalTerrainModel,             
                post_box: &Sender<ocad::Object>,
                verbose: bool) {

    let module = "RAIN".blue();

    if verbose {
        println!("[{}] Applying {} mm of rain to entire map.", &module, RAIN_MM);
    }

    let mut water_per_triangle: Vec<f64> = dtm.areas.iter().map(|a| a*RAIN_M).collect();
    let mut absorbed_water = vec![0f64;dtm.num_triangles];
    let gravity = Point3D { x: 0f64, y: 0f64, z: -1f64, };

    let ratios: Vec<[f64;3]> = dtm.normals().into_iter().enumerate()
        .map(|(t,n)| -> [f64;3] {
            // Calculate resultant vector of normal + gravity.
            let resultant = Point3D { x: n[0], y: n[1], z: n[2] - 1.0f64 };

            let p = [dtm.points[dtm.vertices[t*3+0]], dtm.points[dtm.vertices[t*3+1]], dtm.points[dtm.vertices[t*3+2]]];
            let a = p[0].distance_3d_to(&p[1]);
            let b = p[1].distance_3d_to(&p[2]);
            let c = p[2].distance_3d_to(&p[0]);
            let s = a + b + c;

            let incenter = Point3D {
                x: (a * p[0].x + b * p[1].x + c * p[2].x)/s,
                y: (a * p[0].y + b * p[1].y + c * p[2].y)/s,
                z: (a * p[0].z + b * p[1].z + c * p[2].z)/s,
            };

            let factors: Vec<f64> = (0..3).map(|i: Halfedge| -> f64 {
                let a = p[i];
                let ab = p[i.next()] - a;
                let ap = incenter - a;
                let r = ap.dot(&ab) / ab.dot(&ab);
                let projected = Point3D { x: a.x + r*ab.x, y: a.y + r*ab.y, z: a.z + r*ab.z, };
                let ip = (projected - incenter).normalized();
                
                f64::max(gravity.dot(&ip), 0f64)
            }).collect();
            [factors[0], factors[1], factors[2]]
        })
        .collect();

    for triangle in 0..dtm.num_triangles {
        if dtm.terrain[triangle] == Terrain::Lake { water_per_triangle[triangle] = 0f64; }
    }
   
    let z_lim = dtm.z_limits();
    
    // for t in 0..dtm.num_triangles {
    //     if z_lim[t].0 == z_lim[t].1 /*ratios[t].iter().sum::<f64>() < 0.001 */&& dtm.terrain[t] != Terrain::Lake {
    //         // let p = [dtm.points[dtm.vertices[t*3+0]], dtm.points[dtm.vertices[t*3+1]], dtm.points[dtm.vertices[t*3+2]]];
    //         // println!("{} {:?} {:?}", t, p, ratios[t]);
    //         let p0 = dtm.points[dtm.vertices[t*3]];
    //         let p0_sweref = Sweref {east: p0.x, north: p0.y };
    //         let pob = ocad::Object::point_object(205000, &p0_sweref, 0f64);
    //         post_box.send(pob).expect("Unable to send object");
    //     }
    // }

    // return;

    // let normals = dtm.normals();


    // for (t,z) in z_lim.iter().enumerate().skip(1500000) {

    //     if z.1 - z.0 > 0.6 {
    //         let n = normals[t];
    //         let resultant = Point3D { x: n[0], y: n[1], z: n[2] - 1.0f64 };

    //         let p = [dtm.points[dtm.vertices[t*3+0]], dtm.points[dtm.vertices[t*3+1]], dtm.points[dtm.vertices[t*3+2]]];
    //         let a = p[0].distance_3d_to(&p[1]);
    //         let b = p[1].distance_3d_to(&p[2]);
    //         let c = p[2].distance_3d_to(&p[0]);
    //         let s = a + b + c;

    //         let incenter = Point3D {
    //             x: (a * p[0].x + b * p[1].x + c * p[2].x)/s,
    //             y: (a * p[0].y + b * p[1].y + c * p[2].y)/s,
    //             z: (a * p[0].z + b * p[1].z + c * p[2].z)/s,
    //         };
    //         println!("{} {:?}", t, resultant);
    //         println!("{:?}\n{:?}\n{:?}\n{:?}", p[0] - incenter, p[1] - incenter, p[2]-incenter, ratios[t]);

    //         for i in 0..3 {
    //             println!("------- {} -------", i);
    //             let a = p[i];
    //             println!("\ta: {:?}",a-incenter);
    //             let ab = p[i.next()] - a;
    //             println!("\tab: {:?}",ab);
    //             let ap = incenter - a;
    //             println!("\tap: {:?}",ap);
    //             let r = ap.dot(&ab) / ab.dot(&ab);
    //             println!("\tR: {}", r);
    //             let projected = Point3D { x: a.x + r*ab.x, y: a.y + r*ab.y, z: a.z + r*ab.z, };
    //             println!("\tProjected: {:?}", projected-incenter);
    //             let ip = (projected - incenter).normalized();
    //             println!("\tip: {:?}", ip);
    //             println!("\tGravity dot ip: {}", resultant.dot(&ip));
    //         }
    //         panic!("Dne");
    //     }
    // }

    let mut iterations = 0;
    while water_per_triangle.iter().sum::<f64>() > ITERATE_UNTIL_ONLY_THIS_MUCH_WATER_REMAINS {
        // For each triangle, calculate the flow to neighbouring triangles.
        println!("{} {}", water_per_triangle[45], absorbed_water[45]);
        let mut flow = vec![0f64;dtm.num_triangles];

        for (triangle, water) in water_per_triangle.iter().enumerate() {
            if dtm.terrain[triangle] == Terrain::Lake { continue }
            let r = ratios[triangle];

            for i in 0..3 {
                let outflow = r[i] * water;
                flow[triangle] = flow[triangle] - outflow;
                let o = dtm.opposite(triangle*3 + i);
                if o != EMPTY && dtm.terrain[o/3] != Terrain::Lake {
                    flow[o/3] = flow[o/3] + outflow;
                }
            }
            let absorbed = ABSORBTION_FACTOR * water;
            absorbed_water[triangle] = absorbed_water[triangle] + absorbed;
            flow[triangle] = flow[triangle] - absorbed;
        }

        for (triangle, delta_water) in flow.iter().enumerate() {
            water_per_triangle[triangle] = water_per_triangle[triangle] + delta_water;
        }

        iterations = iterations + 1;
    }

    if verbose {
        println!("[{}] The water has dissipated after {} iterations.", &module, iterations);
    }

    let absorbed_per_sqm: Vec<f64> = absorbed_water.into_iter()
        .zip(dtm.areas.iter())
        .map(|(water, area)| water/area)
        .collect();

    let mut assigned_triangles = vec![0usize;dtm.num_triangles];

    let mut marsh_index = 1;

    for (triangle, absorbed) in absorbed_per_sqm.iter().enumerate() {

        // let p0 = dtm.points[dtm.vertices[triangle*3]];
        // let p0_sweref = Sweref { east: p0.x, north: p0.y };
        // let pob = ocad::Object::point_object(205000, &p0_sweref, 0f64);
        // post_box.send(pob).expect("Unable to send object");

        // continue;

        if  assigned_triangles[triangle] != 0
            || *absorbed < DIFFUSE_MARSH_LOWER_LIMIT
            || dtm.exterior[triangle]
            || dtm.areas[triangle] < MIN_AREA_FOR_SEED 
//            || z_lim[triangle].0 == z_lim[triangle].1
            { continue }

        let marsh_type = if *absorbed < DIFFUSE_MARSH_UPPER_LIMIT { Marsh::Diffuse } else 
                        if *absorbed < NORMAL_MARSH_UPPER_LIMIT { Marsh::Normal } else 
                        { Marsh::Impassable };
        let limits = marsh_type.limits();
        let should_grow = |the_marsh: &Boundary, halfedge: Halfedge| -> bool
            {   let t = halfedge / 3;
                absorbed_per_sqm[t] >= limits.0 && absorbed_per_sqm[t] <= limits.1 && 
                the_marsh.indices_for_each_triangle[t] == 0
            };

        let mut marsh = Boundary {
            halfedges: Vec::new(),
            islands: Vec::new(),
            index: marsh_index,
            dtm: dtm,
            indices_for_each_triangle: &mut assigned_triangles,
        };
        marsh.grow_from_triangle(triangle, &should_grow);

        //marsh.split_into_lake_and_islands();
        // println!("{} {} {}", triangle, absorbed, dtm.areas[triangle]);

        //TODO: Remove islands that are too small, but keep the rest.
        // if marsh.halfedges.len() > 3 {
        // let p0 = dtm.points[dtm.vertices[triangle*3]];
        // let p0_sweref = Sweref { east: p0.x, north: p0.y };
        // let pob = ocad::Object::point_object(205000, &p0_sweref, 0f64);
        // post_box.send(pob).expect("Unable to send object");
        // }
        if marsh.halfedges.len() > 3 {
        
        //println!("{:?} {} {} {:5.2} {:?} {:?}", ratios[triangle], triangle, marsh.halfedges.len(), absorbed, marsh_type, limits.0);
        ocad::post_objects_without_clipping(
            marsh.extract_vertices(), 
            &vec![ocad::GraphSymbol::Fill(marsh_type.symbol())],
            &post_box);
        }
        
        marsh_index = marsh_index + 1;
    }

    if verbose {
        println!("[{}] {} marshes added.", &module, marsh_index - 1);
    }

}