use super::dtm::{DigitalTerrainModel,Z_NORMAL,Halfedge,Point3D};
use super::ocad;
use std::sync::mpsc::Sender;
use super::boundary::Boundary;

pub fn should_grow_cliff(cliff: &Boundary, halfedge: Halfedge) -> bool {
    true
}

pub fn handler(dtm: &DigitalTerrainModel, 
            post_box: Sender<ocad::Object>) {

    const MAX_ALLOWED_EDGE: f64 = 2.0;
    const MAX_ZNORMAL_FOR_SEED: f64 = 0.3f64;

    // Identify seed triangles: edges < 5 m, z-normal < 0.3.

    let seed_triangles: Vec<usize> = dtm.vertices
        .chunks(3)
        .zip(dtm.normals.iter())
        .enumerate()
        .filter_map(|(triangle_index, (i, normal))| {
            let p0 = dtm.points[i[0]];
            let p1 = dtm.points[i[1]];
            let p2 = dtm.points[i[2]];
            if p0.distance_2d_to(&p1) < MAX_ALLOWED_EDGE &&
                p1.distance_2d_to(&p2) < MAX_ALLOWED_EDGE &&
                p2.distance_2d_to(&p0) < MAX_ALLOWED_EDGE &&
                normal[2] < MAX_ZNORMAL_FOR_SEED { Some(triangle_index) } else { None }
        }).collect();

    let mut cliff_index_per_triangle = vec![0 as usize; dtm.num_triangles];
    
    // Take a seed triangle.
    // If it already has a cliff index, skip it.

}