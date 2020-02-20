use super::dtm::{DigitalTerrainModel,Z_NORMAL,Halfedge,Terrain};
use super::ocad;
use std::sync::mpsc::Sender;
use super::boundary::Boundary;
use super::geometry::Plane;

const MAX_ALLOWED_EDGE: f64 = 2.0;
const MAX_ZNORMAL_FOR_SEED: f64 = 0.3f64;
const MAX_ZNORMAL_FOR_GROW: f64 = 0.5f64;
const MAX_ANGLE_TO_VERTICAL: f64 = 30f64;

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
                normal[Z_NORMAL] < MAX_ZNORMAL_FOR_SEED { Some(triangle_index) } else { None }
        }).collect();

    let should_grow_cliff = |cliff: &Boundary, halfedge: Halfedge| -> bool {
        let t = halfedge / 3;
        cliff.indices_for_each_triangle[t] == 0 &&
        normals[t][Z_NORMAL] < MAX_ZNORMAL_FOR_GROW && 
        cliff.dtm.terrain[t] == Terrain::Unclassified &&
        cliff.dtm.length_of_halfedge(halfedge) < MAX_ALLOWED_EDGE
    };

    let mut cliff_index_per_triangle = vec![0 as usize; dtm.num_triangles];
    
    let cliff_index: usize = 1;

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
        let incenters = cliff_index_per_triangle.iter()
            .enumerate()
            .filter_map(|(i,c)| if *c == cliff_index { Some(dtm.triangle_incenter(i)) } else { None })
            .collect();

        // Create plane from incenters and verify that angle to vertical is low enough.
        match Plane::from_points(&incenters) {
            Some(plane) if plane.angle_to_vertical() < MAX_ANGLE_TO_VERTICAL => {
                // curve reconstruction from unorganized points
                // Determine initial P0,P1,P2,P3. P0 and P3 extreme points along intersection
                // of plane with average z. P1 and P2 lie between extreme points. 

                // Solve for t. 

                // Px = P0x * (1-t)^3 ... 
                // Py = P0y * (1-t)^3 ...
                // Third degree polynomial in t. Hopefully only one real solution.
                // rgsl::polynomials::cubic_equations::poly_solve_cubic

                // Solve for each point.
                // Now perform least squares with (1-t)^3, t*(1-t)^2, t^2*(1-t), t^3 as coefficients.
                // Can iterate, and check error after we've calculated t -> how far from actual point?

                // Should we recalculate P0..P3 when done, if there are t values well outside the [0,1] range?


                // http://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.103.6770&rep=rep1&type=pdf

                for (i,c) in cliff_index_per_triangle.iter().enumerate() {
                    if *c == cliff_index {
                        dtm.terrain[i] = Terrain::Cliff;
                    }
                }
            },
            _ => { },
        };
    }

}