use super::Sweref;
use nalgebra::DMatrix;
use super::dtm::Point3D;

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
        self.contains(&other.northwest())
    }

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
    point: Point3D,
    pub average_z: f64,
}

impl Plane {

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
