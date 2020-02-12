use super::dtm::{Point3D, DigitalTerrainModel, Halfedge, TriangleWalk, Z_NORMAL};
use super::ocad;
use colored::*;
use std::sync::mpsc::Sender;
use super::sweref_to_wgs84::Sweref;
use delaunator::EMPTY;
use flo_curves::*;
use ::geo::{Point,Coordinate,LineString};
use ::geo::algorithm::simplifyvw::SimplifyVW;
use ::geo::algorithm::simplify::Simplify;

#[derive(Debug)]
struct Contour {
    linestring: LineString<f64>,
    gradients: Vec<f64>,
    closed: bool,
    base_elevation: f64,
}

fn point_to_sweref(c: &Point<f64>) -> Sweref {
    Sweref { east: c.x(), north: c.y() }
}

fn coord2_to_sweref(c: &Coord2) -> Sweref {
    Sweref { east: c.x(), north: c.y() }
}

impl Contour {

    pub fn simplify(&mut self) {
        // simplify:
        // 5.0 - hyfsat utseende resultat. aningens kantiga.
        // simpl
//        self.linestring = self.linestring.simplify(&5.0);
        self.linestring = self.linestring.simplifyvw(&5.0);
    }

    pub fn ocad_object(&self) -> ocad::Object {
        let segments = self.linestring
            .points_iter()
            .enumerate()
            .map(|x| {
                let s = point_to_sweref(&x.1);
                if x.0 == 0 { ocad::Segment::Move(s) } else { ocad::Segment::Line(s) }
            }).collect();

        ocad::Object {
            object_type: ocad::ObjectType::Line(false),
            symbol_number: 101000,
            segments,
        }
    }

    pub fn bezier_ocad_object(&self) -> ocad::Object {
        let mut segments: Vec<ocad::Segment> = Vec::new();
        let coords: Vec<Coord2> = self.linestring.points_iter().map(|p| Coord2(p.x(),p.y())).collect();
        let beziers: Vec<bezier::Curve<Coord2>> = bezier::fit_curve(&coords[..], 1.0).expect("Unable to create bezier");

        if beziers.len() > 0 {
            segments.push(ocad::Segment::Move(coord2_to_sweref(&beziers[0].start_point)));
        }
        for b in beziers.into_iter() {
            segments.push(ocad::Segment::Bezier(
                coord2_to_sweref(&b.control_points.0),
                coord2_to_sweref(&b.control_points.1),
                coord2_to_sweref(&b.end_point),
            ));
        }

        ocad::Object {
            object_type: ocad::ObjectType::Line(false),
            symbol_number: 101000,
            segments,
        }
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

        let mut remaining_interior_triangles_intersecting_z: Vec<usize> = dtm.z_limits.iter()
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
                if f >= 0.0f64 && f <= 1.0f64 { 
                    Position::OnEdge(h, Point3D { 
                        x: p0.x + f * (p1.x - p0.x),
                        y: p0.y + f * (p1.y - p0.y),
                        z: z, })
                } else { Position::NotOnEdge }
            }
        };

        while let Some(starting_triangle) = remaining_interior_triangles_intersecting_z.pop() {
            let mut points: Vec<Coordinate<f64>> = Vec::new();
            let mut gradients: Vec<f64> = Vec::new();
            let mut reached_first_end_of_open_contour = false;

            // Find the first halfedge that intersects the first triangle.
            let mut halfedge = starting_triangle * 3;
            let mut p = position_on_halfedge(halfedge);
            while p == Position::NotOnEdge {
                halfedge = halfedge.next();
                p = position_on_halfedge(halfedge);
            }
            let mut starting_halfedge: Option<Halfedge> = None;

            loop {
                let current_triangle = halfedge / 3;
                // Get the exit point.
                let exit = match position_on_halfedge(halfedge.next()) {
                    Position::NotOnEdge => position_on_halfedge(halfedge.prev()),
                    x => x,
                };

                match exit { 
                    Position::OnEdge(h, p) => {
                        if starting_halfedge.is_none() { starting_halfedge = Some(h); }
                        points.push(Coordinate { x: p.x, y: p.y, });
                        gradients.push(dtm.normals[current_triangle][Z_NORMAL]);
                        halfedge = dtm.opposite(h);

                        if (halfedge == EMPTY || dtm.exterior[halfedge/3]) && !reached_first_end_of_open_contour {
                            points.reverse();
                            gradients.reverse();
                            halfedge = starting_halfedge.unwrap();
                            reached_first_end_of_open_contour = true;
                        }

                        if (halfedge == EMPTY || dtm.exterior[halfedge/3])  && reached_first_end_of_open_contour {
                            break;
                        }
                    },
                    Position::NotOnEdge => panic!("Exit point is not on the halfedge?"),
                }
                
                if halfedge/3 == starting_triangle {
                    break;
                }

                match remaining_interior_triangles_intersecting_z.iter().position(|x| *x == halfedge/3 ) {
                    Some(pos) => { remaining_interior_triangles_intersecting_z.remove(pos); },
                    None => { 
                        // Problemet är att första trianglen blir kvar i de fall då vi börjar mitt på en öppen kurva.
                        // println!("{} {} missing {} {}\n{} {:?} {}", halfedge/3,current_triangle, starting_triangle, reached_first_end_of_open_contour, halfedge, starting_halfedge, z);
                        // panic!("Still") 
                        break;
                    },
                };

            }
            let closed = !reached_first_end_of_open_contour;

            // println!("Finished {} contour with {} points at z={} m.", if closed { "closed" } else { "open" }, points.len(), z);
            contours.push(Contour {
                linestring: LineString::from(points), gradients, closed, base_elevation: z,
            })
        }

        contours
    }
}

pub fn handler(dtm: &DigitalTerrainModel, 
    min_z: f64, max_z: f64,
    post_box: Sender<ocad::Object>, verbose: bool) {
    let module = "CONTOUR".red();
    let mut z = min_z + 1f64;

    let mut total_contours = 0;
    while z < max_z {
        for mut c in Contour::from_dtm(dtm, z)
            .into_iter() {
            
            c.simplify();
            if let Some(object) = match c.linestring.num_coords() {
                0..=3 => None,
                4..=10 => Some(c.ocad_object()),
                _ => Some(c.bezier_ocad_object()),
            } { 
                post_box.send(object).expect("Unable to send contour!");   
                total_contours = total_contours+1;
            }
        }

        z = z + 5f64;
    }
    
    if verbose {
        println!("[{}] {} contours added.", &module, total_contours);
    }
    
    // Wait for lakes and cliffs, which can alter score of contours.
}