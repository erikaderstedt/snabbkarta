extern crate delaunator;

use delaunator::EMPTY;
use super::ocad;
use std::sync::mpsc::Sender;
use super::las::PointDataRecord;
use super::dtm::{DigitalTerrainModel,Point3D,Halfedge};
use super::boundary::Boundary;

const Z_NORMAL_REQUIREMENT: f64 = 0.9993f64;

pub fn should_grow_lake(lake: &Boundary, halfedge: Halfedge) -> bool {
    true
}


pub fn handler( records: &Vec<PointDataRecord>, record_to_point3D: &dyn Fn(&PointDataRecord) -> Point3D,
            dtm: &DigitalTerrainModel, 
            post_box: Sender<ocad::Object>) {

    let water_points: Vec<Point3D> = records.iter()
        .filter(|record| record.classification == 9)
        .map(record_to_point3D)
        .collect();

    // For each water point, find the matching triangle.

    let mut triangle_indices_for_water_points = vec![EMPTY; water_points.len()];
    let mut triangle = 0;
    let mut lake_indices_for_triangles = vec![0 as usize, dtm.num_triangles];
    
    for i in 0..water_points.len() {
        triangle_indices_for_water_points[i] = match dtm.triangle_containing_point(&water_points[i], triangle) {
            Some(x) => { triangle = x; x },
            None => EMPTY,
        }
    }

    let mut lake_index: usize = 1;

    for i in 0..water_points.len() {
        triangle = triangle_indices_for_water_points[i];
        
        if dtm.normals[triangle][2] >= Z_NORMAL_REQUIREMENT {
            let mut lake = Boundary {
                halfedges: Vec::new(),
                index: lake_index,
                dtm: dtm,
                indices_for_each_triangle: &mut lake_indices_for_triangles,
            };

            lake.grow_from_triangle(triangle, &should_grow_lake);

            if lake.halfedges.len() > 3 {
                
            }

            lake_index = lake_index + 1;
        }
    }

    // Creat
}