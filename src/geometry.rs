use super::Sweref;
use nalgebra::DMatrix;
use crate::las::LAS_File_Header;

pub struct PointConverter {
    x_scale_factor: f64,
    y_scale_factor: f64,
    z_scale_factor: f64,
    x_offset: f64,
    y_offset: f64,
    z_offset: f64,
}

impl PointConverter {

    pub fn record_coordinates_to_point_3d(&self, record_coordinates: &[i32;3]) -> Point3D {
        Point3D {
            x: ((record_coordinates[0] as f64) * self.x_scale_factor + self.x_offset),
            y: ((record_coordinates[1] as f64) * self.y_scale_factor + self.y_offset),
            z: ((record_coordinates[2] as f64) * self.z_scale_factor + self.z_offset),
        }
    }

    pub fn point_3d_to_record_coordinates(&self, point: &Point3D) -> [i32;3] {
        [((point.x - self.x_offset) / self.x_scale_factor) as i32,
        ((point.y - self.y_offset) / self.y_scale_factor) as i32,
        ((point.z - self.z_offset) / self.z_scale_factor) as i32]
    }

    pub fn from(header: &LAS_File_Header) -> PointConverter {
        PointConverter {
            x_scale_factor: header.x_scale_factor,
            x_offset : header.x_offset,
            y_scale_factor : header.y_scale_factor,
            y_offset : header.y_offset,
            z_scale_factor : header.z_scale_factor,
            z_offset : header.z_offset,
        }
    }

    pub fn z_resolution(&self) -> f64 { self.z_scale_factor }

}

#[derive(Copy,Clone,Debug,PartialEq)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3D {
    pub fn to_the_left_of(&self, p0: &Point3D, p1: &Point3D) -> bool {
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

    pub fn distance_3d_to(&self, other: &Point3D) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        f64::sqrt(dx*dx + dy*dy + dz*dz)
    }

    pub fn dot(&self, other: &Point3D) -> f64 {
        self.x*other.x + self.y*other.y + self.z*other.z
    }

    pub fn cross(&self, other: &Point3D) -> Point3D {
        Point3D { 
            x: self.y*other.z - self.z*other.y,
            y: self.x*other.z - self.z*other.x,
            z: self.x*other.y - self.y*other.x,
        }
    }

    pub fn normalized(&self) -> Point3D {
        let f = f64::sqrt(self.dot(self));
        Point3D { x: self.x / f, y: self.y / f, z: self.z / f, }
    }
}

impl std::ops::Sub for Point3D {
    type Output = Point3D;

    fn sub(self, other: Point3D) -> Point3D {
        Point3D { x: self.x - other.x, y: self.y - other.y, z: self.z - other.z, }
    }
}

#[derive(Clone,Debug)]
pub struct Bounds {
    pub lower: Point3D,
    pub upper: Point3D,
}

impl Bounds {
    pub fn contains_2d(&self, point: &Point3D) -> bool {
        self.lower.x <= point.x && point.x <= self.upper.x &&
        self.lower.y <= point.y && point.y <= self.upper.y
    }

    pub fn outset_by(&self, distance: f64) -> Bounds {
        Bounds {
            lower: Point3D { x: self.lower.x - distance, y: self.lower.y - distance, z: self.lower.z - distance, },
            upper: Point3D { x: self.upper.x + distance, y: self.upper.y + distance, z: self.upper.z + distance, },
        }
    }
}

#[derive(Clone,Copy,Debug)]
pub struct Rectangle {
    pub southwest: Sweref, 
    pub northeast: Sweref,
}

impl Rectangle {

    pub fn from_points(pts: &Vec<Sweref>) -> Self {
        Self {
            southwest: Sweref {
                east: pts.iter().map(|p| p.east).fold(0./0., f64::min),
                north: pts.iter().map(|p| p.north).fold(0./0., f64::min),
            },
            northeast: Sweref {
                east: pts.iter().map(|p| p.east).fold(0./0., f64::max),
                north: pts.iter().map(|p| p.north).fold(0./0., f64::max),
            }
        }
    }

    pub fn create(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Self {
        Self {
            southwest: Sweref { east: xmin, north: ymin, },
            northeast: Sweref { east: xmax, north: ymax, },
        }
    }

    pub fn northwest(&self) -> Sweref { Sweref { north: self.northeast.north, east: self.southwest.east } }
    pub fn southeast(&self) -> Sweref { Sweref { north: self.southwest.north, east: self.northeast.east } }

    pub fn middle(&self) -> Sweref { 
        Sweref {
            north: (self.southwest.north + self.northeast.north) * 0.5,
            east: (self.southwest.east + self.northeast.east) * 0.5,
        }
    }
    
    pub fn segments(&self) -> Vec<LineSegment> {

        let nw = self.northwest();
        let se = self.southeast();
        vec![
            LineSegment { p0: self.southwest.clone(), p1: se.clone() },
            LineSegment { p0: se.clone(), p1: self.northeast.clone() },
            LineSegment { p0: self.northeast.clone(), p1: nw.clone() },
            LineSegment { p0: nw.clone(), p1: self.southwest.clone() },
        ]
    }

    pub fn contains(&self, p: &Sweref) -> bool {
        p.east <= self.northeast.east && 
        p.east >= self.southwest.east && 
        p.north >= self.southwest.north &&
        p.north <= self.northeast.north
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.contains(&other.southwest) || 
        self.contains(&other.southeast()) || 
        self.contains(&other.northeast) ||
        self.contains(&other.northwest()) ||
        other.contains(&self.southwest) // In case we are completely inside other.
    }

    pub fn max_x(&self) -> f64 { return self.northeast.east }
    pub fn max_y(&self) -> f64 { return self.northeast.north }
    pub fn min_x(&self) -> f64 { return self.southwest.east }
    pub fn min_y(&self) -> f64 { return self.southwest.north }

}

pub struct LineSegment {
    pub p0: Sweref,
    pub p1: Sweref,
}

impl LineSegment {

    pub fn create(a: &Sweref, b: &Sweref) -> LineSegment {
        LineSegment { p0: a.clone(), p1: b.clone() }
    }

    pub fn intersection_with(&self, other: &LineSegment) -> Option<Sweref> {
        // http://www.cs.swan.ac.uk/~cssimon/line_intersection.html
        let x1 = self.p0.east;
        let x2 = self.p1.east;
        let x3 = other.p0.east;
        let x4 = other.p1.east;
        let y1 = self.p0.north;
        let y2 = self.p1.north;
        let y3 = other.p0.north;
        let y4 = other.p1.north;
        
        let n = (x4 - x3)*(y1 - y2) - (x1 - x2)*(y4 - y3);

        let ta = ((y3 - y4)*(x1 - x3) + (x4 - x3)*(y1 - y3))/n;
        let tb = ((y1 - y2)*(x1 - x3) + (x2 - x1)*(y1 - y3))/n;

        // if n was zero, 
        if ta.is_infinite() { return None };

        let intersects = 0.0f64..1.0f64;
        match intersects.contains(&ta) && intersects.contains(&tb) {
            true => Some(Sweref { north: y1 + ta * (y2 - y1), east: x1 + ta * (x2 - x1) }),
            false => None,
        }
    }
}

pub struct Plane {
    normal: [f64;3],
    pub point: Point3D,
    pub average_z: f64,
}

impl Plane {

    pub fn normal_as_point(&self) -> Point3D {
        Point3D { x: self.normal[0], y: self.normal[1], z: self.normal[2], }
    }

    pub fn z_normal(&self) -> f64 { self.normal[2] }

    pub fn from_points(points: &Vec<Point3D>) -> Option<Self> {
        /*  β1x + β2y + β3 = z
            | x1 y1 1 |
            | x2 y2 1 |
        X = | x3 y3 1 |  (M = number of points, N = 3)
            | x4 y4 1 |
            | x5 y5 1 |
            | z1 |
            | z1 |
        Y = | z1 | (M = number of points, N = 1)
            | z1 |
            | z1 |
            | F1 |
        β = | F2 | (M = 3, N = 1)
            | F3 |
        Y = Xβ
        OLS: β_opt = (XT X)^(-1) XT Y
        3xC * Cx3 => 3x3
        3x3 * 3xC => 3xC
        3xC * Cx1 => 3x1
        */
        if points.len() < 3 { return None }

        let y = DMatrix::from_iterator(points.len(),1,points.iter().map(|p| p.z));    
        let x = DMatrix::from_iterator(3,points.len(),points.iter().map(|p| vec![p.x, p.y, 1f64]).flatten()).transpose();
        
        let (q,r) = x.qr().unpack();
        if let Some(i) = r.try_inverse() {
            let b = (i * q.transpose() * y).normalize();
            let avg_x = points.iter().map(|p| p.x).sum::<f64>()/(points.len() as f64);
            let avg_y = points.iter().map(|p| p.y).sum::<f64>()/(points.len() as f64);
            let average_z = points.iter().map(|p| p.z).sum::<f64>()/(points.len() as f64);
            let z = b[0]*avg_x + b[1]*avg_y + b[2];
            Some(Plane { 
                normal: [b[0],b[1],b[2]], 
                point: Point3D { x: avg_x, y: avg_y, z, },
                average_z, })
        } else {
            None
        }
    }

    pub fn angle_to_vertical(&self) -> f64 {
        f64::acos(self.normal[2]).to_degrees()
    }

    pub fn intersection_with_z(&self, z: f64) -> (Point3D,Point3D) {
        let x0 = self.point.x;
        let y0 = (z - self.normal[2] - self.normal[0]*x0)/self.normal[1];
        let x = self.normal[0];
        let y = self.normal[1];
        (Point3D {x: -y + x0, y: x + y0, z, },
         Point3D {x: y + x0, y: -x + y0, z, })
    }

} 
