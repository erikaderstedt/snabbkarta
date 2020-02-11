use super::dtm::{Point3D, DigitalTerrainModel, Halfedge, TriangleWalk, Z_NORMAL};
use super::ocad;
use std::sync::mpsc::Sender;
use super::sweref_to_wgs84::Sweref;
use delaunator::EMPTY;

struct Contour {
    points: Vec<Point3D>,
    gradients: Vec<f64>,
    base_elevation: f64,
}

impl Contour {
    pub fn as_sweref(&self) -> Vec<Sweref> {
        self.points.iter()
            .map(|p| Sweref { east: p.x, north: p.y })
            .collect()
    }
}

#[derive(PartialEq)]
enum Position {    
    OnEdge(Halfedge,Point3D),
    NotOnEdge,
}

impl Contour {
    fn from_dtm(dtm: &DigitalTerrainModel, z: f64) -> Vec<Contour> {

        let mut contours: Vec<Contour> = Vec::new();

        let mut triangle_indices_encompassing_z: Vec<usize> = dtm.z_limits.iter()
            .zip(dtm.exterior.iter())
            .enumerate()
            .filter_map(|(i,x)| { if z >= (x.0).0 && z <= (x.0).1 && !x.1 { Some(i) } else { None } })
            .collect();

            // This closure should return a point on edge, for exactly two of the
        // three edges in the triangle.
        let position_on_halfedge = |h: Halfedge| -> Position {
            let p0 = dtm.points[dtm.vertices[h]];
            let p1 = dtm.points[dtm.vertices[h.next()]];
            if p0.z == z { 
                Position::OnEdge(h, p0)
            } else if p1.z == z { 
                Position::OnEdge(h, p1)
            } else {
                let f = (z - p0.z) / (p1.z - p0.z);
                match f {
                    0.0..=1.0 => Position::OnEdge(h, Point3D { 
                                x: p0.x + f * (p1.x - p0.x),
                                y: p0.y + f * (p1.y - p0.y),
                                z: z,
                                }),
                    _ => Position::NotOnEdge,
                }
            }
        };

        while triangle_indices_encompassing_z.len() > 0 {
            let mut points: Vec<Point3D> = Vec::new();
            let mut gradients: Vec<f64> = Vec::new();
            let mut halfedges: Vec<usize> = Vec::new();
            let mut reached_first_end_of_open_contour = false;

            // Find the first relevant halfedge.
            let starting_triangle: usize = triangle_indices_encompassing_z[0];
            let mut halfedge = starting_triangle*3;
            let mut p = position_on_halfedge(halfedge);
            while p == Position::NotOnEdge {
                halfedge = halfedge.next();
                p = position_on_halfedge(halfedge);
            }

            loop {
                let current_triangle = halfedge / 3;
                // Get the exit point.
                let exit = match position_on_halfedge(halfedge.next()) {
                    Position::NotOnEdge => position_on_halfedge(halfedge.prev()),
                    x => x,
                };

                match exit { 
                    Position::OnEdge(h, p) => {
                        halfedges.push(h);
                        points.push(p);
                        gradients.push(dtm.normals[current_triangle][Z_NORMAL]);

                        match triangle_indices_encompassing_z.iter().position(|x| *x == current_triangle ) {
                            Some(pos) => { triangle_indices_encompassing_z.remove(pos); },
                            None => { },
                        };

                        halfedge = dtm.opposite(h);

                        if (halfedge == EMPTY || dtm.exterior[halfedge/3]) && !reached_first_end_of_open_contour {
                            points.reverse();
                            gradients.reverse();
                            halfedge = halfedges[0];
                            reached_first_end_of_open_contour = true;
                        }

                        if (halfedge == EMPTY || dtm.exterior[halfedge/3])  && reached_first_end_of_open_contour {
                            break;
                        }
                    },
                    Position::NotOnEdge => panic!("Exit point is not on the halfedge?"),
                }
                
                // if the next triangle is the same as the starting triangle,
                // finish the contour.
                if halfedge/3 == starting_triangle {
                    break;
                }
            }

            contours.push(Contour {
                points, gradients, base_elevation: z,
            })    
        }

        contours
    }
}

pub fn handler(dtm: &DigitalTerrainModel, 
    min_z: f64, max_z: f64,
    post_box: Sender<ocad::Object>) {

    let mut z = min_z + 1f64;

    while z < max_z {
        let c = Contour::from_dtm(dtm, z)
        .iter()
        .map(|c| c.as_sweref() )
        .collect();

        ocad::post_objects_without_clipping(c, &vec![ocad::GraphSymbol::Stroke(101000, false)], &post_box);

        z = z + 5f64;
    }

    
    // Wait for lakes and cliffs, which can alter score of contours.
}