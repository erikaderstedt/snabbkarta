use super::dtm::{Point3D, DigitalTerrainModel,TriangleWalk,Halfedge, Terrain,Z_NORMAL};
use super::ocad;
use colored::*;
use std::sync::mpsc::{channel,Receiver,Sender};
use std::thread;
use std::sync::Arc;
use std::ops::Deref;
use super::sweref_to_wgs84::Sweref;
use delaunator::EMPTY;
use flo_curves::*;
use ::geo::{Point,Coordinate,LineString};
use ::geo::algorithm::simplifyvw::SimplifyVW;
use ::geo::algorithm::euclidean_length::EuclideanLength;
use std::cmp::Ordering;

const BASE_EQUIDISTANCE: f64 = 5f64;
const CONTOUR_STEP: f64 = 0.5f64;

const PENALTY_FOR_ADJACENT_TO_LAKE: f64 = 200f64;
const BONUS_FOR_ON_CLIFF: f64 = 50f64;
const DESIRED_LENGTH_FOR_CLOSED_CONTOUR: f64 = 30f64;
const PENALTY_FOR_TOO_SHORT_CLOSED_CONTOUR: f64 = 20f64;
const PENALTY_FOR_EASILY_SIMPLIFIED: f64 = 100f64;
const LIMIT_FOR_EASILY_SIMPLIFIED: f64 = 0.2f64;

#[derive(Debug)]
pub struct Contour {
    linestring: LineString<f64>,
    triangles: Vec<usize>,
    closed: bool,
    original_length: f64,
    base_elevation: f64,
}

fn point_to_sweref(c: &Point<f64>) -> Sweref {
    Sweref { east: c.x(), north: c.y() }
}

fn coord2_to_sweref(c: &Coord2) -> Sweref {
    Sweref { east: c.x(), north: c.y() }
}

impl Contour {
    pub fn score(&self, dtm: &DigitalTerrainModel, normals: &Vec<[f64;3]>) -> f64 {
        let mut score = 0f64;
        let length = self.linestring.euclidean_length();

        // "Not getting anywhere". Length of simplified contour is < 60% of original.
        if length / self.original_length < LIMIT_FOR_EASILY_SIMPLIFIED {
            score = score - PENALTY_FOR_EASILY_SIMPLIFIED;
        }

        for t in self.triangles.iter() {
            for i in 0..3 {
                let o = dtm.opposite(t*3 + i);
                if o != EMPTY && dtm.terrain[o/3] == Terrain::Lake {
                    score = score - PENALTY_FOR_ADJACENT_TO_LAKE;
                }
            }
            if dtm.terrain[*t] == Terrain::Cliff { 
                score = score + BONUS_FOR_ON_CLIFF;
            }

            let f = (normals[*t][Z_NORMAL] - 1f64)*2.0 - 1.0;
            score = score + f*f*f*2000.0;
        }

        if self.closed && length < DESIRED_LENGTH_FOR_CLOSED_CONTOUR {
            score = score - PENALTY_FOR_TOO_SHORT_CLOSED_CONTOUR;
        }

        score = score + length;
        

        


        // TODO: Gradients - the most important aspect!

        // TODO: Inflection points
        // Window of 7 points. 
        // 4 outer points - identify Bezier.
        // Sum of distance to 3 inner points must be large enough. 
        // Middle point is inflection point. 

        score
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
        let beziers: Vec<bezier::Curve<Coord2>> = bezier::fit_curve(&coords[..], 5.0).expect("Unable to create bezier");

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

type Position = (Halfedge,Point3D);

impl Contour {
    fn from_dtm(dtm: &DigitalTerrainModel, z_limits: &Vec<(f64,f64)>, z: f64) -> Vec<Contour> {
        let intersections_with_triangle = |t: usize| -> Vec<Position> {
            ((t*3)..(t*3+3)).filter_map(|i| {
                let a = dtm.points[dtm.vertices[i]];
                let b = dtm.points[dtm.vertices[i.next()]];

                if a.z == z { Some((i,a)) } 
                else if a.z == b.z { None } 
                else {
                    let f = (z - a.z) / (b.z - a.z);
                    if f > 0.0 && f < 1.0 {
                        Some((i, Point3D { x: a.x + f*(b.x-a.x), y: a.y + f*(b.y-a.y), z, }))
                    } else {
                        None
                    }
                }
            }).collect()
        };

        let mut remaining_interior_triangles_intersecting_z: Vec<usize> = z_limits.iter()
            .zip(dtm.exterior.iter())
            .enumerate()
            .filter_map(|(i,((min,max),exterior))| { 
                if z >= *min && z <= *max && 
                !exterior && 
                min != max &&
                intersections_with_triangle(i).len() == 2
                { Some(i) } else { None } })
            .collect();

        let mut contours: Vec<Contour> = Vec::new();

        // If a contour follows a lake edge (or edges of any triangles with horizontal bottom or top edges), it may branch. 
        // This can be avoided by not using the same z:s as are used in the LAS data.

        while remaining_interior_triangles_intersecting_z.len() > 0 {
            let starting_triangle = remaining_interior_triangles_intersecting_z[0];
            let mut halfedge = EMPTY;
            let mut triangle = starting_triangle;
            let mut triangles: Vec<usize> = Vec::new();
            let mut points: Vec<Coordinate<f64>> = Vec::new();
            let mut reached_first_end_of_open_contour = false;

            assert_eq!(intersections_with_triangle(starting_triangle).len(), 2, "Not exactly two intersects in start");

            loop {
                
                let intersects = intersections_with_triangle(triangle);
                halfedge = match intersects.len() {
                    1 => { dtm.opposite(intersects[0].0).next() },
                    2 => {
                        match remaining_interior_triangles_intersecting_z.iter().position(|x| *x == triangle ) {
                            Some(pos) => { remaining_interior_triangles_intersecting_z.remove(pos); },
                            None => { break }, // A branch occured 
                        };
                        let (exit_halfedge, p) = intersects.iter().filter(|(h,_)| *h != halfedge).next().expect("No intersection matches!");
                        points.push(Coordinate { x: p.x, y: p.y, });
                        triangles.push(triangle);
                        dtm.opposite(*exit_halfedge)
                    },
                    3 => { panic!("A contour intersected a completely flat triangle!"); },
                    _ => { panic!("Weird number of intersects!") },
                };

                // If opposite is empty, or exterior, turn.
                if halfedge == EMPTY || dtm.exterior[halfedge/3] {
                    if reached_first_end_of_open_contour {
                        break;
                    } else {
                        points.reverse();
                        triangles.reverse();
                        halfedge = intersections_with_triangle(starting_triangle)[0].0;
                        remaining_interior_triangles_intersecting_z.push(starting_triangle);
                        reached_first_end_of_open_contour = true;
                    }
                }

                triangle = halfedge/3;

                if triangle == starting_triangle && !reached_first_end_of_open_contour {
                    break;
                }
            }

            let closed = !reached_first_end_of_open_contour;
            let npoints = points.len();
            let original_linestring = LineString::from(points);
            let original_length = original_linestring.euclidean_length();
            let linestring = original_linestring.simplifyvw(&5.0);
            if npoints >= 2 && original_length > 0f64 {
                contours.push(Contour { linestring, triangles, closed, original_length, base_elevation: z, })
            }
        }
        contours
    }
}

pub fn create_contours_from_base_z(dtm: Arc<DigitalTerrainModel>, normals: Arc<Vec<[f64;3]>>,
    min_z: f64, max_z: f64, offset: f64,
    post_box: Sender<(f64,f64,Vec<Contour>)>) {

    let mut contours: Vec<Contour> = Vec::new();
    let mut z = min_z + offset;

    let z_limits = dtm.z_limits();

    while z < max_z {
        contours.append(&mut Contour::from_dtm(&dtm.deref(), &z_limits, z));
        z = z + BASE_EQUIDISTANCE;
    }
    let score = contours.iter()
        .map(|c| c.score(&dtm.deref(), &normals.deref())).sum::<f64>();

    post_box.send((offset, score, contours)).expect("Unable to send contours to collator!");
}


pub fn create_contours(dtm: DigitalTerrainModel, 
    min_z: f64, max_z: f64, z_resolution: f64,
    post_box: Sender<ocad::Object>, verbose: bool) {
    let module = "CONTOUR".red();

    let (tx, rx): (Sender<(f64,f64,Vec<Contour>)>, Receiver<(f64,f64,Vec<Contour>)>) = channel();
    let normals = dtm.normals();
    let dtm_rc = Arc::new(dtm);
    let normals_rc = Arc::new(normals);
    let mut offset: f64 = z_resolution*0.5;
    let mut num_contour_levels = 0;
    while offset < BASE_EQUIDISTANCE - CONTOUR_STEP*0.5 {
        let d = dtm_rc.clone();
        let n = normals_rc.clone();
        let collector_box = tx.clone();
        thread::spawn(move || {
            create_contours_from_base_z(d, n, min_z, max_z, offset, collector_box);            
        }); 
        offset = offset + CONTOUR_STEP;
        num_contour_levels = num_contour_levels + 1;
    }

    let mut contour_sets = Vec::new();
    while num_contour_levels > 0 {
        contour_sets.push(rx.recv().expect("Unable to receive contour data"));
        num_contour_levels = num_contour_levels - 1;
    }

    if verbose {
        println!("[{}] Created {} contours at 0.5 m intervals.", &module, 
            contour_sets.iter().map(|s| s.2.len()).sum::<usize>());
    }

    contour_sets.sort_by(|a,b| if a.1 > b.1 { Ordering::Less } else { Ordering::Greater });
    // for c in contour_sets.iter() {
    //     println!("{} {} {}", c.0, c.1, c.2.len());
    // }

    let (level, _, contours) = &contour_sets[0];
    println!("Choosing {}, with {} contours.", level, contours.len());
    let mut total_contours = 0;

    for c in contours.iter() {
            if let Some(object) = match c.linestring.num_coords() {
                0..=3 => None,
                4..=100000 => Some(c.ocad_object()),
                _ => Some(c.bezier_ocad_object()),
            } { 
                post_box.send(object).expect("Unable to send contour!");   
                total_contours = total_contours+1;
            }
        }
    
    if verbose {
        println!("[{}] {} contours added.", &module, total_contours);
    }
}