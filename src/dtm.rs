use crate::las::PointDataRecord;
use delaunator::{Point,triangulate,EMPTY};
use std::f64;
use crate::geometry::{Point3D,Bounds,PointConverter};

pub const Z_NORMAL: usize = 2;

pub type Halfedge = usize;

pub trait TriangleWalk {
    fn next(&self) -> Halfedge;
    fn prev(&self) -> Halfedge;
}

impl TriangleWalk for Halfedge {
    fn next(&self) -> Halfedge {
        if *self == EMPTY { EMPTY } else {
            match self % 3 {
                2 => self - 2,
                _ => self + 1,
            }
        }
    }
    fn prev(&self) -> Halfedge {
        if *self == EMPTY { EMPTY } else {
            match self % 3 {
            0 => self + 2,
            _ => self - 1,
            }
        }
    }    
}

#[derive(Clone,PartialEq)]
pub enum Terrain {
    Unclassified,
    Lake,
    Cliff,
}

#[derive(Clone)]
pub struct DigitalTerrainModel {
    pub points: Vec<super::Point3D>,
    // "vertices" contains the point indices for each triangle, in chunks of three
    // Indices 0..3 are the points for the first triangle, as indices into "points".
    pub vertices: Vec<usize>,
    // Halfedges contains the opposite halfedge of each halfedge, or "EMPTY" if the 
    // halfedge is on the convex hull (outer edge) of the Delauney triangulation.
    pub halfedges: Vec<Halfedge>,
    pub num_triangles: usize,
    pub areas: Vec<f64>,
    pub exterior: Vec<bool>,
    pub terrain: Vec<Terrain>,
    pub bounds: Bounds,
}

impl DigitalTerrainModel {

    pub fn triangle_incenter(&self, triangle: usize) -> Point3D {
        let p0 = self.points[self.vertices[triangle*3+0]];
        let p1 = self.points[self.vertices[triangle*3+1]];
        let p2 = self.points[self.vertices[triangle*3+2]];

        let a = p0.distance_2d_to(&p1);
        let b = p1.distance_2d_to(&p2);
        let c = p2.distance_2d_to(&p0);
        let s = a + b + c;

        Point3D {
            x: (a * p0.x + b * p1.x + c * p2.x)/s,
            y: (a * p0.y + b * p1.y + c * p2.y)/s,
            z: (a * p0.z + b * p1.z + c * p2.z)/s,
        }
    }

    pub fn opposite(&self, h: Halfedge) -> Halfedge {
        self.halfedges[h]
    }

    pub fn length_of_halfedge(&self, h: Halfedge) -> f64 {
        let p0 = self.points[self.vertices[h]];
        let p1 = self.points[self.vertices[h.next()]];
        p0.distance_2d_to(&p1)
    }

    pub fn create(records: &Vec<PointDataRecord>, point_converter: &PointConverter) -> DigitalTerrainModel {

        let ground_points: Vec<Point3D> = records.iter()
            .filter(|record| record.classification == 2)
            .map(|record| point_converter.record_coordinates_to_point_3d(&[record.x, record.y, record.z]))
            .collect();

        let gp_delaunator: Vec<Point> = ground_points.iter().map(|p| Point { x: p.x, y: p.y, }).collect();
    
        let triangulation = triangulate(&gp_delaunator[..]).expect("No triangulation exists.");
        let num_triangles = triangulation.triangles.len() / 3;
        const MARGIN: f64 = 5.0;

        let max_x = ground_points.iter().map(|p| p.x).fold(0./0., f64::max) - MARGIN;
        let min_x = ground_points.iter().map(|p| p.x).fold(0./0., f64::min) + MARGIN;
        let max_y = ground_points.iter().map(|p| p.y).fold(0./0., f64::max) - MARGIN;
        let min_y = ground_points.iter().map(|p| p.y).fold(0./0., f64::min) + MARGIN;
        let max_z = ground_points.iter().map(|p| p.z).fold(0./0., f64::max);
        let min_z = ground_points.iter().map(|p| p.z).fold(0./0., f64::min);

        let bounds = Bounds { lower: Point3D { x: min_x, y: min_y, z: min_z }, upper: Point3D { x: max_x, y: max_y, z: max_z } };
        
        let exteriors = triangulation.triangles
            .chunks(3)
            .map(|i| [&ground_points[i[0]], &ground_points[i[1]], &ground_points[i[2]]])
            .enumerate()
            .map(|(i,p)| {
                p[0].x < min_x || p[1].x < min_x || p[2].x < min_x ||
                p[0].x > max_x || p[1].x > max_x || p[2].x > max_x ||
                p[0].y < min_y || p[1].y < min_y || p[2].y < min_y ||
                p[0].y > max_y || p[1].y > max_y || p[2].y > max_y ||
                triangulation.halfedges[i*3] == EMPTY ||
                triangulation.halfedges[i*3+1] == EMPTY ||
                triangulation.halfedges[i*3+2] == EMPTY
            }).collect();

        let areas = triangulation.triangles
            .chunks(3)
            .map(|i| [&ground_points[i[0]], &ground_points[i[1]], &ground_points[i[2]]])
            .map(|p| { f64::abs(0.5*(p[0].x * (p[1].y - p[2].y) +
                                     p[1].x * (p[2].y - p[0].y) +
                                     p[2].x * (p[0].y - p[1].y)))
            }).collect();

        DigitalTerrainModel {
            points: ground_points,
            vertices: triangulation.triangles.clone(),
            halfedges: triangulation.halfedges.clone(),
            num_triangles: num_triangles,
            terrain: vec![Terrain::Unclassified; num_triangles],
            exterior: exteriors, areas: areas, bounds
        }
    }

    pub fn normals(&self) -> Vec<[f64;3]> {
        self.vertices.chunks(3)
            .map(|i| [&self.points[i[0]], &self.points[i[1]], &self.points[i[2]]])
            .map(|p| {
                let v = Point3D { x: p[1].x-p[0].x, y: p[1].y-p[0].y, z: p[1].z-p[0].z };
                let u = Point3D { x: p[2].x-p[0].x, y: p[2].y-p[0].y, z: p[2].z-p[0].z };
                let nx = u.y*v.z - u.z*v.y;
                let ny = u.z*v.x - u.x*v.z;
                let nz = u.x*v.y - u.y*v.x;
                let l = f64::sqrt(nx*nx + ny*ny + nz*nz);
                [nx/l, ny/l, nz/l]
            }).collect()
    }

    pub fn z_limits(&self) -> Vec<(f64,f64)> {
        self.vertices.chunks(3)
            .map(|i| [&self.points[i[0]], &self.points[i[1]], &self.points[i[2]]])
            .map(|p| {
                
                let zs = [p[0].z, p[1].z, p[2].z];
                (zs.iter().cloned().fold(0./0., f64::min), 
                 zs.iter().cloned().fold(0./0., f64::max))
            }).collect()
    }

    fn next_triangle_toward_point(&self, point: &Point3D, triangle: usize) -> Option<usize> {
        for edge in 0..3 {
            let p0 = &self.points[self.vertices[triangle*3 + edge]];
            let p1 = &self.points[self.vertices[triangle*3 + ((edge+1)%3)]];
            if point.to_the_left_of(p0,p1) {
                let r = self.halfedges[triangle*3 + edge];
                return match r {
                    EMPTY => None,
                    _ => Some(r / 3),
                };
            }
        }
        Some(triangle)
    }

    pub fn triangle_containing_point(&self, point: &Point3D, previous: usize) -> Option<usize> {
        let mut triangle = previous;
        loop {
            triangle = match self.next_triangle_toward_point(point, triangle) {
                Some(t) if t != triangle => t,
                x => { return x },
            }
        }
    }

    pub fn z_coordinate_at_xy(&self, point: &Point3D) -> f64 {
        match self.triangle_containing_point(point, 0usize) {
            None => (self.bounds.upper.z + self.bounds.lower.z) * 0.5f64,
            Some(triangle) => {
                let p0 = self.points[self.vertices[triangle*3+0]];
                let p1 = self.points[self.vertices[triangle*3+1]];
                let p2 = self.points[self.vertices[triangle*3+2]];

                let v = Point3D { x: p1.x-p0.x, y: p1.y-p0.y, z: p1.z-p0.z };
                let u = Point3D { x: p2.x-p0.x, y: p2.y-p0.y, z: p2.z-p0.z };
                let nx = u.y*v.z - u.z*v.y;
                let ny = u.z*v.x - u.x*v.z;
                let nz = u.x*v.y - u.y*v.x;
                let l = f64::sqrt(nx*nx + ny*ny + nz*nz);
                let n = [nx/l, ny/l, nz/l];

                if n[2] == 0f64 {
                    // Vertical triangle
                    (p0.z + p1.z + p2.z) * 0.33f64
                } else {
                    // d = n[0]*p0.x + n[1]*p0.y + n[2]*p0.z
                    // d = n[0]*point.x + n[1]*point.y + n[2]*point.z
                    (n[0]*p0.x + n[1]*p0.y + n[2]*p0.z - n[0]*point.x - n[1]*point.y) / n[2]
                }
            }
        }
    }

}
