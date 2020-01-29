extern crate delaunator;

use delaunator::{Triangulation, EMPTY};
use super::ocad;
use std::sync::mpsc::{channel,Receiver,Sender};
use super::las;

impl Point3D {

    fn to_the_left_of(&self, p0: &Point3D, p1: &Point3D) -> bool {
        let vx = p1.x - p0.x;
        let vy = p1.y - p0.y;
        let toselfx = self.x - p0.x;
        let toselfy = self.y - p0.y;
        vx*toselfy - vy*toselfx > 0
    }

}

#[derive(Copy,Clone)]
struct DigitalTerrainModel {
    points: Vec<super::Point3D>,
    triangles: Vec<usize>,
    halfedges: Vec<usize>,
    num_triangles: usize,
    normals: Vec<[f64;3]>,
    exterior: Vec<bool>,
}

struct TriangleIterator {
    dtm: &DigitalTerrainModel,
    triangle_index: usize,
}

impl Iterator for TriangleIterator {
    type Item = [super::Point3D;3];
    fn next(&mut self) -> Option<Item> {
        match self.triangle_index {
            self.num_triangles => None,
            i => vec![
                dtm.points[dtm.triangles[i*3]], 
                dtm.points[dtm.triangles[i*3+1]], 
                dtm.points[dtm.triangles[i*3+2]],
            ],
        }
    }
}

impl DigitalTerrainModel {

    fn triangle_iter<'a>(&self) -> TriangleIterator {
        TriangleIterator { dtm: &'a self, triangle_index: 0, } }
    }

    fn create(records: &Vec<las::PointDataRecord>, record_to_point3D: F) -> DigitalTerrainModel 
        where F: Fn(&las::PointDataRecord) -> super::Point3D {

        let ground_points = records.iter().
            .filter(|record| record.classification == 2)
            .map(record_to_point3D)
            .collect();
    
        let triangulation = triangulate(&ground_points).expect("No triangulation exists.");
        let num_triangles = triangulation.triangles / 3;
        const MARGIN: f64 = 5.0;

        let max_x = ground_points.iter().max_by_key(|p| p.x).expect("No ground points!") - MARGIN;
        let min_x = ground_points.iter().min_by_key(|p| p.x).expect("No ground points!") + MARGIN;
        let max_y = ground_points.iter().max_by_key(|p| p.y).expect("No ground points!") - MARGIN;
        let min_y = ground_points.iter().min_by_key(|p| p.y).expect("No ground points!") + MARGIN;
        
        DigitalTerrainModel {
            points: ground_points,
            triangles: triangulation.triangles.clone(),
            halfedges: triangulation.halfedges.clone(),
            num_triangles: num_triangles,
            normals: 0..num_triangles.iter().map(|i| {
                let p0 = ground_points[triangulation.triangles[i*3]];
                let p1 = ground_points[triangulation.triangles[i*3+1]];
                let p2 = ground_points[triangulation.triangles[i*3+2]];
                let v = Point3D { x: p1.x-p0.x, y: p1.y-p0.y, z: p1.z-p0.z };
                let u = Point3D { x: p2.x-p0.x, y: p2.y-p0.y, z: p2.z-p0.z };
                let nx = u.y*v.z - u.z*v.y;
                let ny = u.z*v.x - u.x*v.z;
                let nz = u.x*v.y - u.y*v.x;
                let l = f64::sqrt(nx*nx + ny*ny + nz*nz);
                vec![nx/l, ny/l, nz/l]
            }),
            exterior: 0..num_triangles.iter().map(|i| {
                let p0 = ground_points[triangulation.triangles[i*3]];
                let p1 = ground_points[triangulation.triangles[i*3+1]];
                let p2 = ground_points[triangulation.triangles[i*3+2]];
                p0.x < min_x || p1.x < min_x || p2.x < min_x ||
                p0.x > max_x || p1.x > max_x || p2.x > max_x ||
                p0.y < min_y || p1.y < min_y || p2.y < min_y ||
                p0.y > max_y || p1.y > max_y || p2.y > max_y ||
                triangulation.halfedges[i*3] == EMPTY ||
                triangulation.halfedges[i*3+1] == EMPTY ||
                triangulation.halfedges[i*3+2] == EMPTY
            }),
        }

    }

    fn next_triangle_toward_point(&self, point: &Point3D, triangle: usize) -> Option<usize> {
        for edge in 0..3 {
            let p0 = dtm.points[dtm.triangulation.vertices[triangle*3 + edge]];
            let p1 = dtm.points[dtm.triangulation.vertices[triangle*3 + ((edge+1)%3)]];
            if point.to_the_left_of(p0,p1) {
                let r = dtm.triangulation.halfedges[triangle*3 + edge];
                return match r {
                    EMPTY => None,
                    _ => Some(r) / 3,
                };
            }
        }
        triangle
    };

    fn triangle_containing_point(&self, point: &Point3D, previous: usize) -> Option<usize> {
        let mut triangle = previous;
        loop {
            triangle = match self.next_triangle_toward_point(point, triangle) {
                Some(t) if t != triangle => t,
                x => { return x },
            }
        }
    }
}



fn handler( records: &Vec<las::PointDataRecord>, record_to_point3D: F,
            dtm: &DigitalTerrainModel, 
            post_box: Sender<ocad::Object>) 
        where F: Fn(&las::PointDataRecord) -> super::Point3D {

    let water_points: Vec<Point3D> = records.iter().
        .filter(|record| record.classification == 9)
        .map(record_to_point3D)
        .collect();

    // For each water point, find the matching triangle.

    let mut triangle_indices_for_water_points = vec![EMPTY; water_points.len()];
    let mut triangle = 0;
    let mut lake_indices_for_triangles = vec![0 as usize, dtm.num_triangles];
    
    for i in 0..water_points.len() {
        triangle_indices_for_water_points[i] = match dtm.triangle_containing_point(water_points[i], triangle) {
            Some(x) => { triangle = x; x },
            None => EMPTY,
        }
    }

    for i in 0..water_points.len() {
        triangle = triangle_indices_for_water_points[i];
        if triangle == EMPTY || 
        
    }

        

    // Creat
}