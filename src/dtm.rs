extern crate delaunator;

use super::las::PointDataRecord;
use delaunator::{Point,triangulate,EMPTY};

#[derive(Copy,Clone)]
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
}

pub struct DigitalTerrainModel {
    pub points: Vec<super::Point3D>,
    pub triangles: Vec<usize>,
    pub halfedges: Vec<usize>,
    pub num_triangles: usize,
    pub normals: Vec<[f64;3]>,
    pub exterior: Vec<bool>,
}

struct TriangleIterator <'a> {
    dtm: &'a DigitalTerrainModel,
    triangle_index: usize,
}

impl Iterator for TriangleIterator <'_> {
    type Item = [super::Point3D;3];

    fn next(&mut self) -> Option<Self::Item> {
        let nt: usize = self.dtm.num_triangles;
        let r = match self.triangle_index {
            nt => None,
            i => Some([
                self.dtm.points[self.dtm.triangles[i*3]], 
                self.dtm.points[self.dtm.triangles[i*3+1]], 
                self.dtm.points[self.dtm.triangles[i*3+2]],
            ]),
        };
        self.triangle_index = self.triangle_index + 1;
        r
    }
}

impl DigitalTerrainModel {

    fn triangle_iter<'a>(&'a self) -> TriangleIterator {
        TriangleIterator { dtm: self, triangle_index: 0, }
    }

    pub fn create(records: &Vec<PointDataRecord>, record_to_point3D: &dyn Fn(&PointDataRecord) -> Point3D) -> DigitalTerrainModel {

        let ground_points: Vec<Point3D> = records.iter()
            .filter(|record| record.classification == 2)
            .map(record_to_point3D)
            .collect();

        let gp_delaunator: Vec<Point> = ground_points.iter().map(|p| Point { x: p.x, y: p.y, }).collect();
    
        let triangulation = triangulate(&gp_delaunator[..]).expect("No triangulation exists.");
        let num_triangles = triangulation.triangles.len() / 3;
        const MARGIN: f64 = 5.0;

        let normals = (0..num_triangles).map(|i| {
            let p0 = &ground_points[triangulation.triangles[i*3]];
            let p1 = &ground_points[triangulation.triangles[i*3+1]];
            let p2 = &ground_points[triangulation.triangles[i*3+2]];
            let v = Point3D { x: p1.x-p0.x, y: p1.y-p0.y, z: p1.z-p0.z };
            let u = Point3D { x: p2.x-p0.x, y: p2.y-p0.y, z: p2.z-p0.z };
            let nx = u.y*v.z - u.z*v.y;
            let ny = u.z*v.x - u.x*v.z;
            let nz = u.x*v.y - u.y*v.x;
            let l = f64::sqrt(nx*nx + ny*ny + nz*nz);
            [nx/l, ny/l, nz/l]
        }).collect();

        let max_x = ground_points.iter().map(|p| p.x).fold(0./0., f64::max) - MARGIN;
        let min_x = ground_points.iter().map(|p| p.x).fold(0./0., f64::min) + MARGIN;
        let max_y = ground_points.iter().map(|p| p.y).fold(0./0., f64::max) - MARGIN;
        let min_y = ground_points.iter().map(|p| p.y).fold(0./0., f64::min) + MARGIN;

        let exterior = (0..num_triangles).map(|i| {
            let p0 = &ground_points[triangulation.triangles[i*3]];
            let p1 = &ground_points[triangulation.triangles[i*3+1]];
            let p2 = &ground_points[triangulation.triangles[i*3+2]];
            p0.x < min_x || p1.x < min_x || p2.x < min_x ||
            p0.x > max_x || p1.x > max_x || p2.x > max_x ||
            p0.y < min_y || p1.y < min_y || p2.y < min_y ||
            p0.y > max_y || p1.y > max_y || p2.y > max_y ||
            triangulation.halfedges[i*3] == EMPTY ||
            triangulation.halfedges[i*3+1] == EMPTY ||
            triangulation.halfedges[i*3+2] == EMPTY
        }).collect();
        
        DigitalTerrainModel {
            points: ground_points,
            triangles: triangulation.triangles.clone(),
            halfedges: triangulation.halfedges.clone(),
            num_triangles: num_triangles,
            normals: normals,
            exterior: exterior,
        }

    }

    fn next_triangle_toward_point(&self, point: &Point3D, triangle: usize) -> Option<usize> {
        for edge in 0..3 {
            let p0 = &self.points[self.triangles[triangle*3 + edge]];
            let p1 = &self.points[self.triangles[triangle*3 + ((edge+1)%3)]];
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
