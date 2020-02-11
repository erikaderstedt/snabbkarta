use delaunator::EMPTY;
use colored::*;
use super::ocad;
use std::sync::mpsc::Sender;
use super::las::PointDataRecord;
use super::dtm::{DigitalTerrainModel,Point3D,Halfedge, Z_NORMAL};
use super::boundary::Boundary;

const Z_NORMAL_REQUIREMENT: f64 = 0.9993f64;
const TRIANGLE_CONTAINS_WATER_POINT: usize = 0x80000000;
const LAKE_INDEX_MASK: usize = 0x7fffffff;

pub fn should_grow_lake(lake: &Boundary, halfedge: Halfedge) -> bool {
    let triangle = halfedge / 3;
    lake.indices_for_each_triangle[triangle] & LAKE_INDEX_MASK == 0 && // Not already claimed.triangle
        (lake.dtm.normals[triangle][Z_NORMAL] >= Z_NORMAL_REQUIREMENT ||
        (lake.dtm.length_of_halfedge(halfedge) > 5.0 && !lake.dtm.exterior[triangle]) || // TODO: also not exterior
        lake.indices_for_each_triangle[triangle] & TRIANGLE_CONTAINS_WATER_POINT > 0)
}

pub fn handler( records: &Vec<PointDataRecord>, record_to_point_3d: &dyn Fn(&PointDataRecord) -> Point3D,
            dtm: &DigitalTerrainModel, 
            post_box: Sender<ocad::Object>) {

    let module = "LAKE".blue();

    let water_points: Vec<Point3D> = records.iter()
        .filter(|record| record.classification == 9)
        .map(record_to_point_3d)
        .collect();

    println!("[{}] Creating lakes from {} water points.", &module, water_points.len());

    // For each water point, find the matching triangle.
    // Maintain two lists: one with the triangle index for a water point.
    // Another with the lake index for each triangle. If any. Also, remember
    // if the triangle contains a water point. Keep that in the top bit.

    let mut triangle_indices_for_water_points = vec![EMPTY; water_points.len()];
    let mut triangle = 0;
    let mut lake_indices_for_triangles = vec![0 as usize; dtm.num_triangles];
    
    for i in 0..water_points.len() {
        triangle_indices_for_water_points[i] = match dtm.triangle_containing_point(&water_points[i], triangle) {
            Some(x) => { 
                triangle = x; 
                lake_indices_for_triangles[x] = TRIANGLE_CONTAINS_WATER_POINT;
                x },
            None => EMPTY,
        }
    }

    let mut lake_index: usize = 1;
    let mut actual_lakes = 0;

    for i in 0..water_points.len() {
        triangle = triangle_indices_for_water_points[i];
        
        if triangle == EMPTY || (lake_indices_for_triangles[triangle] & LAKE_INDEX_MASK) != 0 { 
            continue; 
        }

        if dtm.normals[triangle][2] >= Z_NORMAL_REQUIREMENT {
            let mut lake = Boundary {
                halfedges: Vec::new(),
                islands: Vec::new(),
                index: lake_index,
                dtm: dtm,
                indices_for_each_triangle: &mut lake_indices_for_triangles,
            };

            lake.grow_from_triangle(triangle, &should_grow_lake);
            if lake.halfedges.len() > 3 {
                lake.split_into_lake_and_islands();
                ocad::post_objects_without_clipping(
                    lake.extract_vertices(), 
                    &vec![ocad::GraphSymbol::Fill(301002)],
                    &post_box);

                actual_lakes = actual_lakes + 1;

                let mut border = Vec::new();
                border.append(&mut lake.extract_interior_segments(&lake.halfedges));
                for i in lake.islands.iter() {
                    border.append(&mut lake.extract_interior_segments(i));
                }
                ocad::post_objects_without_clipping(
                    border, 
                    &vec![ocad::GraphSymbol::Stroke(301001, false)],
                    &post_box);
            }

            lake_index = lake_index + 1;
        }
    }
    println!("[{}] Found {} lakes.", &module, actual_lakes);

}