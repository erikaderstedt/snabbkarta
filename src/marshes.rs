use super::dtm::{DigitalTerrainModel,Z_NORMAL,Halfedge,Terrain,TriangleWalk, Point3D};
use super::ocad;
use std::sync::mpsc::Sender;
use super::boundary::Boundary;
use super::geometry::Plane;
use std::cmp::Ordering;
use ::geo::{Coordinate,LineString};
use ::geo::algorithm::simplifyvw::SimplifyVW;
use ::geo::algorithm::euclidean_length::EuclideanLength;
use super::Sweref;
use colored::*;
use std::f64;

const Z_NORMAL_REQUIREMENT: f64 = 0.9999f64;
const MIN_AREA_FOR_SEED: f64 = 0.6f64;
const MAX_ANGLE_TO_VERTICAL: f64 = 0.001f64;
const MAX_Z_DIFF: f64 = 0.4f64;
const MIN_AREA_FOR_OUTPUT: f64 = 20f64;

pub fn detect_marshes_in(dtm: &DigitalTerrainModel, post_box: &Sender<ocad::Object>, center: &Sweref, verbose: bool) {

    let module = "MARSH".blue();

    if verbose {
        println!("[{}] Detecting marshes.", &module);
    }

    let should_grow = |marsh: &Boundary, halfedge: Halfedge| -> bool {
        let t = halfedge/3;
        
        if marsh.indices_for_each_triangle[t] != 0 || marsh.dtm.terrain[t] != Terrain::Unclassified { 
            return false; 
        }

        let opposing_point = marsh.dtm.points[marsh.dtm.vertices[halfedge.prev()]];
        // Angle should still be flat
        // z value of triangle should not deviate too much from average z.
        let p = marsh.halfedges.iter().map(|h| marsh.dtm.points[marsh.dtm.vertices[*h]] ).collect();
        //let average_z = p.iter().map(|p| p.z).sum::<f64>()/p.len();

        match Plane::from_points(&p) {
            Some(plane) => {
                plane.angle_to_vertical() < MAX_ANGLE_TO_VERTICAL
//                plane.z_normal() > Z_NORMAL_REQUIREMENT
                && 
                f64::abs((opposing_point - plane.point).dot(&plane.normal_as_point())) < MAX_Z_DIFF
            },
            _ => p.len() <= 3,
        }
    };

    let mut marsh_index_per_triangle = vec![0 as usize; dtm.num_triangles];
    let mut marsh_index = 1;
    let mut num_marshes_output = 0;
    let mut total_area_of_marshes = 0f64;

    for (triangle, normal) in dtm.normals().into_iter().enumerate().take(500000) {
        if  marsh_index_per_triangle[triangle] != 0
            || dtm.terrain[triangle] != Terrain::Unclassified
            || normal[Z_NORMAL] < Z_NORMAL_REQUIREMENT
            || dtm.areas[triangle] < MIN_AREA_FOR_SEED
            || dtm.exterior[triangle]
            { continue }
        
            // let p = [dtm.points[dtm.vertices[triangle*3+0]], dtm.points[dtm.vertices[triangle*3+1]], dtm.points[dtm.vertices[triangle*3+2]]];
            // println!("Seed {} {:?} {} {}",triangle, p, dtm.areas[triangle], normal[Z_NORMAL]);

        // Start growing
        // We need extra stuff for marsh.

        let mut marsh = Boundary {
            halfedges: Vec::new(),
            islands: Vec::new(),
            index: marsh_index,
            dtm: dtm,
            indices_for_each_triangle: &mut marsh_index_per_triangle,
        };

        marsh.grow_from_triangle(triangle, &should_grow);
        //marsh.split_into_lake_and_islands();

        if marsh.halfedges.len() > 10 {
            let area = marsh.indices_for_each_triangle.iter().filter(|i| **i == marsh_index).map(|i| dtm.areas[*i]).sum::<f64>();
            if area > MIN_AREA_FOR_OUTPUT {
                ocad::post_objects_without_clipping(
                    marsh.extract_vertices(), 
                    &vec![ocad::GraphSymbol::Fill(406000)],
                    &post_box);
                total_area_of_marshes = total_area_of_marshes + area;
                num_marshes_output = num_marshes_output + 1;
                println!("{} {} {}", triangle, marsh.halfedges.len(), area);
            }
        }
       
        marsh_index = marsh_index + 1;
    }
    if verbose {
        println!("[{}] {} marshes detected, total area {:.0} m².", &module, num_marshes_output, total_area_of_marshes);
    }

}