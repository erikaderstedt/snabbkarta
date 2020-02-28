use super::dtm::{DigitalTerrainModel,Terrain,Point3D,Halfedge,TriangleWalk};
use delaunator::EMPTY;

pub fn rain_on(dtm: &mut DigitalTerrainModel) {

    let mut water_per_triangle: Vec<f64> = dtm.areas.clone();
    let mut integrated_water_per_triangle = vec![0f64;dtm.num_triangles];
    let gravity = Point3D { x: 0f64, y: 0f64, z: -1f64, };

    let ratios: Vec<[f64;3]> = (0..dtm.num_triangles)
        .map(|t| -> [f64;3] {
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
                gravity.dot(&ip)
            }).collect();
            [factors[0], factors[1], factors[2]]
        })
        .collect();

    for iteration in 0..100 {
        println!("Iteration {}, {} m^3 of water.", iteration, water_per_triangle.sum()*0.01);
        // For each triangle, calculate the flow to neighbouring triangles.
        let mut flow = vec![0f64;dtm.num_triangles];

        for (triangle, water) in water_per_triangle.iter().enumerate() {
            if dtm.terrain[triangle] == Terrain::Lake { continue }
            let r = ratios[triangle];

            for i in 0..3 {
                let outflow = r[i] * water;
                flow[triangle] = flow[triangle] - outflow;
                match dtm.opposite(triangle + i) {
                    EMPTY => {},
                    o => { 
                        let t = o/3;
                        if dtm.terrain[t] != Terrain::Lake { flow[t] = flow[t] + outflow }
                    },
                };
            }
        }

        for (triangle, delta_water) in flow.iter().enumerate() {
            water_per_triangle[triangle] = water_per_triangle[triangle] + delta_water;
            integrated_water_per_triangle[triangle] = integrated_water_per_triangle[triangle] + water_per_triangle[triangle];
        }
    }

    // Filter out triangles with enough water - these are seed triangles for heavy bogs. 
    for (triangle, water) in integrated_water_per_triangle.iter().enumerate() {
        // Is the triangle already claimed?
        
    }
        .filter()
    }
    // .. include islands but not smaller than 4 triangles.



    // Now do intermediate bogs

    // Diffuse bogs.

}