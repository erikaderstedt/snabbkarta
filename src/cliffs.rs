use super::dtm::{DigitalTerrainModel,Z_NORMAL,Halfedge,Terrain, Point3D};
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

const MAX_ALLOWED_EDGE: f64 = 10.0;
const MAX_ZNORMAL_FOR_SEED: f64 = 0.75f64;
const MAX_ZNORMAL_FOR_GROW: f64 = 0.65f64;
const MAX_ANGLE_TO_VERTICAL: f64 = 70f64;
const MIN_REQUIRED_HEIGHT: f64 = 1.2f64;
const MIN_REQUIRED_Z_DIFF: f64 = 0.5f64;
const UNPASSABLE_CLIFF: f64 = 2f64;

pub fn detect_cliffs(dtm: &mut DigitalTerrainModel, 
            post_box: &Sender<ocad::Object>,
            verbose: bool) {

    // Identify seed triangles: edges < 5 m, z-normal < 0.3.
    let normals = dtm.normals();
    let z_limits = dtm.z_limits();
    let seed_triangles: Vec<usize> = dtm.vertices
        .chunks(3)
        .zip(normals.iter())
        .enumerate()
        .filter_map(|(triangle_index, (i, normal))| {
            let p0 = dtm.points[i[0]];
            let p1 = dtm.points[i[1]];
            let p2 = dtm.points[i[2]];
            if p0.distance_2d_to(&p1) < MAX_ALLOWED_EDGE &&
                p1.distance_2d_to(&p2) < MAX_ALLOWED_EDGE &&
                p2.distance_2d_to(&p0) < MAX_ALLOWED_EDGE &&
                z_limits[triangle_index].1 - z_limits[triangle_index].0 > MIN_REQUIRED_Z_DIFF &&
                !dtm.exterior[triangle_index] &&
                normal[Z_NORMAL] < MAX_ZNORMAL_FOR_SEED { Some(triangle_index) } else { None }
        }).collect();

    let should_grow_cliff = |cliff: &Boundary, halfedge: Halfedge| -> bool {
        let t = halfedge / 3;
        cliff.indices_for_each_triangle[t] == 0 &&
        normals[t][Z_NORMAL] < MAX_ZNORMAL_FOR_GROW && 
        !cliff.dtm.exterior[t] &&
        z_limits[t].1 - z_limits[t].0 > MIN_REQUIRED_Z_DIFF &&
        cliff.dtm.terrain[t] == Terrain::Unclassified &&
        cliff.dtm.length_of_halfedge(halfedge) < MAX_ALLOWED_EDGE
    };

    let mut cliff_index_per_triangle = vec![0 as usize; dtm.num_triangles];
    
    let mut cliff_index: usize = 1;
    let mut num_cliffs_output = 0;

    for seed_triangle in seed_triangles.into_iter() {
        // Take a seed triangle.
        // If it already has a cliff index, skip it.
        if cliff_index_per_triangle[seed_triangle] != 0 { continue };

        let mut cliff = Boundary {
            halfedges: Vec::new(),
            islands: Vec::new(),
            index: cliff_index,
            dtm: dtm,
            indices_for_each_triangle: &mut cliff_index_per_triangle,
        };

        cliff.grow_from_triangle(seed_triangle, &should_grow_cliff);
        cliff.split_into_lake_and_islands();

        let height = {
            let (z_min, z_max) = cliff.halfedges.iter()
                .fold((f64::MAX,f64::MIN), |z, h| {
                    let p = dtm.points[dtm.vertices[*h]];
                    (   if p.z < z.0 { p.z } else { z.0 },
                        if p.z > z.1 { p.z } else { z.1 })
                });
            z_max - z_min
        };

        if height > MIN_REQUIRED_HEIGHT && cliff.halfedges.len() > 3 {

            let incenters: Vec<Point3D> = cliff.indices_for_each_triangle.iter()
            .enumerate()
            .filter_map(|(i,c)| if *c == cliff_index { 
                Some(dtm.triangle_incenter(i)) 
            } else { 
                None 
            })
            .collect();

            // Create plane from incenters and verify that angle to vertical is low enough.
            match Plane::from_points(&incenters) {
                Some(plane) if plane.angle_to_vertical() < MAX_ANGLE_TO_VERTICAL => {

                                    // Sort points along projection onto intersection with average z.
                let (a,b) = plane.intersection_with_z(plane.average_z);
                let ab = b - a;
                let mut projections: Vec<(f64, Coordinate<f64>)> = incenters.iter()
                    .map(|p| {
                        let ap = *p - a;
                        (ap.dot(&ab) / ab.dot(&ab), Coordinate { x: p.x, y: p.y, })
                    }).collect();

                projections.sort_by(|a,b| if a.0 < b.0 { Ordering::Less } else { Ordering::Greater });
                let ordered_points: Vec<Coordinate<f64>> = projections.into_iter().map(|(_,p)| p).collect();
                let linestring = LineString::from(ordered_points).simplifyvw(&5.0);
                if linestring.euclidean_length() > 4.0 { 
                    // curve reconstruction from unorganized points is non-trivial.
                    // I've experimented a bit with it but without success.

                    let segments = linestring
                        .points_iter()
                        .enumerate()
                        .map(|x| {
                            let s: Sweref = Sweref::from(&x.1);
                            if x.0 == 0 { ocad::Segment::Move(s) } else { ocad::Segment::Line(s) }
                        }).collect();

                        
                        // let segments = incenters
                        //     .iter()
                        //     .enumerate()
                        //     .map(|x| {
                        //         let s: Sweref = Sweref { east: x.1.x, north: x.1.y, };
                        //         if x.0 == 0 { ocad::Segment::Move(s) } else { ocad::Segment::Line(s) }
                        //     }).collect();

                                
                        post_box.send(ocad::Object {
                            object_type: ocad::ObjectType::Line(false),
                            symbol_number: if height > UNPASSABLE_CLIFF { 201000 } else { 202000 },
                            segments,
                        }).expect("Unable to send cliff!");

                        num_cliffs_output = num_cliffs_output + 1;

                        for (i,c) in cliff_index_per_triangle.iter().enumerate() {
                            if *c == cliff_index {
                                dtm.terrain[i] = Terrain::Cliff;
                            }
                        }
                    }
                },
                _ => {},            
            };
        }

        cliff_index = cliff_index + 1;
        // // Create plane from incenters and verify that angle to vertical is low enough.
        // match Plane::from_points(&incenters) {
        //     Some(plane) 
        //         if plane.angle_to_vertical() < MAX_ANGLE_TO_VERTICAL && 
        //             height > MIN_REQUIRED_HEIGHT => {

        //         // Sort points along projection onto intersection with average z.
        //         let (a,b) = plane.intersection_with_z(plane.average_z);
        //         let ab = b - a;
        //         let mut projections: Vec<(f64, Coordinate<f64>)> = incenters.iter()
        //             .map(|p| {
        //                 let ap = *p - a;
        //                 (ap.dot(&ab) / ab.dot(&ab), Coordinate { x: p.x, y: p.y, })
        //             }).collect();

        //         projections.sort_by(|a,b| if a.0 < b.0 { Ordering::Less } else { Ordering::Greater });
        //         let ordered_points: Vec<Coordinate<f64>> = projections.into_iter().map(|(_,p)| p).collect();
        //         let linestring = LineString::from(ordered_points).simplifyvw(&5.0);

        //         // curve reconstruction from unorganized points is non-trivial.
        //         // I've experimented a bit with it but without success.

        //         // let segments = linestring
        //         //     .points_iter()
        //         //     .enumerate()
        //         //     .map(|x| {
        //         //         let s: Sweref = Sweref::from(&x.1);
        //         //         if x.0 == 0 { ocad::Segment::Move(s) } else { ocad::Segment::Line(s) }
        //         //     }).collect();

                    
        //         let segments = incenters
        //         .iter()
        //         .enumerate()
        //         .map(|x| {
        //             let s: Sweref = Sweref { east: x.1.x, north: x.1.y, };
        //             if x.0 == 0 { ocad::Segment::Move(s) } else { ocad::Segment::Line(s) }
        //         }).collect();

                    
        //         //println!("{:?}", segments);
        //         post_box.send(ocad::Object {
        //             object_type: ocad::ObjectType::Line(false),
        //             symbol_number: if height > UNPASSABLE_CLIFF { 201000 } else { 202000 },
        //             segments,
        //         }).expect("Unable to send cliff!");
        //         outputted_cliffs = outputted_cliffs + 1;
        //         for (i,c) in cliff_index_per_triangle.iter().enumerate() {
        //             if *c == cliff_index {
        //                 dtm.terrain[i] = Terrain::Cliff;
        //             }
        //         }
        //     },
        //     _ => { },
        // };
    }
    if verbose {
        let module = "CLIFF".black();
        println!("[{}] {} cliffs created.", &module, num_cliffs_output);
    }
}