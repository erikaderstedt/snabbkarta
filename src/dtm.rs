use super::las::PointDataRecord;
use delaunator::{Point,triangulate,EMPTY};
use std::f64;

pub const Z_NORMAL: usize = 2;

pub type Halfedge = usize;

pub trait TriangleWalk {
    fn next(&self) -> Halfedge;
    fn prev(&self) -> Halfedge;
}

impl TriangleWalk for Halfedge {
    fn next(&self) -> Halfedge {
        match self % 3 {
            2 => self - 2,
            _ => self + 1,
        }
    }
    fn prev(&self) -> Halfedge {
        match self % 3 {
            0 => self + 2,
            _ => self - 1,
        }
    }    
}

#[derive(Copy,Clone,Debug,PartialEq)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3D {
    fn to_the_left_of(&self, p0: &Point3D, p1: &Point3D) -> bool {
        let vx = p1.x - p0.x;
        let vy = p1.y - p0.y;
        let toselfx = self.x - p0.x;
        let toselfy = self.y - p0.y;
        vx*toselfy - vy*toselfx > 0f64
    }

    pub fn distance_2d_to(&self, other: &Point3D) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        f64::sqrt(dx*dx + dy*dy)
    }
}

#[derive(Clone)]
pub struct DigitalTerrainModel {
    pub points: Vec<super::Point3D>,
    pub vertices: Vec<usize>,
    pub halfedges: Vec<Halfedge>,
    pub num_triangles: usize,
    pub normals: Vec<[f64;3]>,
    pub areas: Vec<f64>,
    pub exterior: Vec<bool>,
    pub z_limits: Vec<(f64,f64)>,
}

impl DigitalTerrainModel {

    pub fn opposite(&self, h: Halfedge) -> Halfedge {
        self.halfedges[h]
    }

    pub fn length_of_halfedge(&self, h: Halfedge) -> f64 {
        let p0 = self.points[self.vertices[h]];
        let p1 = self.points[self.vertices[h.next()]];
        p0.distance_2d_to(&p1)
    }

    pub fn create(records: &Vec<PointDataRecord>, record_to_point_3d: &dyn Fn(&PointDataRecord) -> Point3D) -> DigitalTerrainModel {

        let ground_points: Vec<Point3D> = records.iter()
            .filter(|record| record.classification == 2)
            .map(record_to_point_3d)
            .collect();

        let gp_delaunator: Vec<Point> = ground_points.iter().map(|p| Point { x: p.x, y: p.y, }).collect();
    
        let triangulation = triangulate(&gp_delaunator[..]).expect("No triangulation exists.");
        let num_triangles = triangulation.triangles.len() / 3;
        const MARGIN: f64 = 5.0;

        let max_x = ground_points.iter().map(|p| p.x).fold(0./0., f64::max) - MARGIN;
        let min_x = ground_points.iter().map(|p| p.x).fold(0./0., f64::min) + MARGIN;
        let max_y = ground_points.iter().map(|p| p.y).fold(0./0., f64::max) - MARGIN;
        let min_y = ground_points.iter().map(|p| p.y).fold(0./0., f64::min) + MARGIN;

        
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
            .map(|p| {
                f64::abs((p[0].x * (p[1].y - p[2].y) +
                p[1].x * (p[2].y - p[0].y) +
                p[2].x * (p[0].y - p[1].y)) * 0.5)
            }).collect();

        let mut dtm = DigitalTerrainModel {
            points: ground_points,
            vertices: triangulation.triangles.clone(),
            halfedges: triangulation.halfedges.clone(),
            num_triangles: num_triangles,
            normals: Vec::new(), exterior: exteriors, areas: areas, z_limits: Vec::new(),
        };

        dtm.recalculate_zlimits_and_normals();
        dtm
    }

    pub fn recalculate_zlimits_and_normals(&mut self) {
        self.normals = self.vertices
            .chunks(3)
            .map(|i| [&self.points[i[0]], &self.points[i[1]], &self.points[i[2]]])
            .map(|p| {
                let v = Point3D { x: p[1].x-p[0].x, y: p[1].y-p[0].y, z: p[1].z-p[0].z };
                let u = Point3D { x: p[2].x-p[0].x, y: p[2].y-p[0].y, z: p[2].z-p[0].z };
                let nx = u.y*v.z - u.z*v.y;
                let ny = u.z*v.x - u.x*v.z;
                let nz = u.x*v.y - u.y*v.x;
                let l = f64::sqrt(nx*nx + ny*ny + nz*nz);
                [nx/l, ny/l, nz/l]
            }).collect();

        self.z_limits = self.vertices
            .chunks(3)
            .map(|i| [&self.points[i[0]], &self.points[i[1]], &self.points[i[2]]])
            .map(|p| {
                
                let zs = [p[0].z, p[1].z, p[2].z];
                (zs.iter().cloned().fold(0./0., f64::min), 
                 zs.iter().cloned().fold(0./0., f64::max))
            }).collect();
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
}