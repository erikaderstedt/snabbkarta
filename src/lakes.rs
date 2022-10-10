use delaunator::EMPTY;
use colored::*;
use super::ocad;
use std::sync::mpsc::Sender;
use super::las::PointDataRecord;
use crate::geometry::{Point3D,PointConverter};
use super::dtm::{DigitalTerrainModel,Halfedge,Terrain,Z_NORMAL};
use super::boundary::{Boundary,extract_vertices,extract_interior_segments};

const Z_NORMAL_REQUIREMENT: f64 = 0.9993f64;
const TRIANGLE_CONTAINS_WATER_POINT: usize = 0x80000000;
const LAKE_INDEX_MASK: usize = 0x7fffffff;

struct Lake<'a> {
    halfedges: Vec<Halfedge>,
    index: usize,
    dtm: &'a DigitalTerrainModel,
    indices_for_each_triangle: &'a mut Vec<usize>,

    normals: &'a Vec<[f64;3]>,
}

impl<'a> Boundary for Lake<'a> {
    fn claim(&mut self, triangle: usize) { self.indices_for_each_triangle[triangle] = self.index; }
    fn push_halfedge(&mut self, h: Halfedge) { self.halfedges.push(h); }
    fn dtm(&self) -> &DigitalTerrainModel { self.dtm }
    fn get_halfedges(&self) -> &Vec<Halfedge> { &self.halfedges }

    fn should_recurse(&self, halfedge: Halfedge) -> bool {
        let triangle = halfedge / 3;
        self.indices_for_each_triangle[triangle] & LAKE_INDEX_MASK == 0 && 
        self.dtm.terrain[triangle] == Terrain::Unclassified &&
            (self.normals[triangle][Z_NORMAL] >= Z_NORMAL_REQUIREMENT ||
            (self.dtm.length_of_halfedge(halfedge) > 5.0 && !self.dtm.exterior[triangle]) || // TODO: also not exterior
            self.indices_for_each_triangle[triangle] & TRIANGLE_CONTAINS_WATER_POINT > 0)
    }
}

pub fn find_lakes( records: &Vec<PointDataRecord>, point_converter: &PointConverter,
            dtm: &mut DigitalTerrainModel, 
            post_box: &Sender<ocad::Object>,
            verbose: bool) {

    let module = "LAKE".blue();
    let normals = dtm.normals();
    let z_resolution = point_converter.z_resolution();

    let water_points: Vec<Point3D> = records.iter()
        .filter(|record| record.classification == 9)
        .map(|record| point_converter.record_coordinates_to_point_3d(&[record.x, record.y, record.z]))
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

    let z_limits = dtm.z_limits();

    for i in 0..water_points.len() {
        triangle = triangle_indices_for_water_points[i];
        
        if  triangle == EMPTY || 
            (lake_indices_for_triangles[triangle] & LAKE_INDEX_MASK) != 0 ||
            normals[triangle][Z_NORMAL] < Z_NORMAL_REQUIREMENT {
            continue; 
        }

        let mut lake = Lake {
            halfedges: Vec::new(),
            index: lake_index,
            dtm: dtm,
            indices_for_each_triangle: &mut lake_indices_for_triangles,
            normals: &normals,
        };

        lake.grow_from_seed(triangle);

        if lake.halfedges.len() > 3 {
            let (main, islands) = lake.split_into_outer_edge_and_islands();
            
            ocad::post_objects_without_clipping(
                extract_vertices(dtm, &main, &islands), 
                &vec![ocad::GraphSymbol::Fill(301002)],
                &post_box);

            actual_lakes = actual_lakes + 1;

            let mut border = Vec::new();
            border.append(&mut extract_interior_segments(dtm, &main));
            for i in islands.iter() {
                border.append(&mut extract_interior_segments(dtm, i));
            }
            ocad::post_objects_without_clipping(
                border, 
                &vec![ocad::GraphSymbol::Stroke(301001, false)],
                &post_box);            
        }

        // Alter the dtm so that the z value of all lake triangles is the median z value of the lake.
        let triangles_for_this_lake: Vec<usize> = lake_indices_for_triangles.iter()
            .enumerate()
            .filter(|i| *(i.1) == lake_index)
            .map(|i| i.0)
            .collect();

        let mut average_z: Vec<f64> = triangles_for_this_lake.iter().map(|i| {
            let (min,max) = z_limits[*i];
            (min+max)*0.5
        }).collect();

        average_z.sort_by(|a,b| if a < b { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater });
        
        let median_of_average_z = average_z[if average_z.len() > 2 { average_z.len()/2 } else { 0 }];
        let m = f64::round(median_of_average_z/z_resolution)*z_resolution;
        for i in triangles_for_this_lake {
            dtm.points[dtm.vertices[i*3]].z = m;
            dtm.points[dtm.vertices[i*3+1]].z = m;
            dtm.points[dtm.vertices[i*3+2]].z = m;
            dtm.terrain[i] = Terrain::Lake;
        }

        lake_index = lake_index + 1;
    }

    if verbose {
        println!("[{}] Found {} lakes.", &module, actual_lakes);
    }
}